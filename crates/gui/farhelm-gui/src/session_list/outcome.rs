//! What a session says about its own operations beyond "in flight": the refusal of the last one, the notice a
//! successful one left, and how long a Restart or a Replace has been waiting, on its row and in the open session's
//! header.
//!
//! SPEC.md, "Lifecycle operations": "A failure or partial result appears on the source session's row", and "After about
//! two seconds waiting for the reply, the sidebar, session header and interrupted-session card share a spinner, the
//! host and elapsed seconds, updated about once a second". Ported from the old app's one error slot per session
//! (`errors` in `session_ops.rs`, filled by `list/view.rs` with "Stop: …" and "Replace: …"), its delete-notice band, and
//! its `LaunchProgress` (`launch_progress.rs`) at the reference commit. Here's the TLDR of the rules:
//!
//! - **One line per session, shown until the session's next operation starts.** This is the one statement of that
//!   rule; the row, its menu and each operation's module link here. The line is the operation store's slot for the
//!   session ([`crate::operations::Operations::failure`]): a refusal, or a partial result a success carried (a Stop
//!   that could not close a tab, a Replace whose source's Delete reported something), which the operation's completion
//!   records there with [`crate::operations::Operations::note`]. The store keeps either until the key's next start or
//!   a reset, whoever starts that work, so an editor or a later header needs no call here. Both are drawn alike, in the
//!   refusal style, as the old app's one error slot drew them. A Stop, Replace, Restart or seen-toggle line carries its
//!   operation's name in front of the words, as that slot did, and a Rename or Delete refusal shows the words alone, as
//!   before. The open session's header shows the same line below its top row ([`pane_line`]), as the old header's
//!   error lines did.
//! - **The wait clock is deadlines, never a timer of its own.** One clock serves Replace and Restart, the two requests
//!   SPEC.md gives this line ("Restarting on <host>…" or "Replacing on <host>…"), and a session has at most one, since
//!   it has at most one request in flight. Nothing shows for the first two seconds; then the line shows the whole
//!   seconds since the request started, moved on once a second by a chain of [`ticker::after`] deadlines, which fire
//!   whether or not the window is active (a wait is about the user's time, not about drawing; SPEC_impl.md, "Animation
//!   time: the ticker"). Each step checks that the work the clock was started for is still in flight on the session, so
//!   the line hides the moment the request settles and the clock itself goes at its next step.
//! - **One UI session.** A clock shows only while the current operation store has its work in flight, and every step
//!   checks the same, so after a reset (SPEC_impl.md, "One failure path: the reset") a clock kept from before shows
//!   nothing and goes at its next step: the new store has none of the old requests in flight, and a request started
//!   under it starts that key's clock over ([`start_clock`]). So the clocks need no keying on the store.
//!
//! The words and lines are functions of a session's key, so the row and the open session's header show the same ones.

use std::collections::HashMap;
use std::time::Duration;

use farhelm_proto::ui_channel::SessionRow;
use gpui::{
    App, AppContext as _, Context, Div, ElementId, Entity, Global, Hsla, InteractiveElement,
    ParentElement, Role, SharedString, Stateful, StatefulInteractiveElement as _, Styled,
    WeakEntity, div, prelude::FluentBuilder as _, px,
};

use super::delete;
use crate::host_text::{HostSentence, HostText};
use crate::operations::{Failure, OpKey, Work};
use crate::theme::{self, metrics};
use crate::{session, ticker};

/// How long a Restart or a Replace waits before its line shows: the old `LaunchProgress`'s two whole seconds, so a quick
/// reply adds nothing.
pub(crate) const PROGRESS_SHOWS_AFTER: Duration = Duration::from_secs(2);

/// How often the Replace clock moves on once it shows.
const PROGRESS_STEP: Duration = Duration::from_secs(1);

// ---- The words -------------------------------------------------------------------------------------------------------

/// The name a line about `work` carries in front of the host's words on the row, as the old app's error slot did:
/// "Stop: ", "Replace: " and "Restart: ", for a refusal and a notice alike, and "Seen: " for a refused Mark read or Mark
/// unread (the old app's `seen_toggle_report`). A Rename or a Delete refusal shows the words alone, as it did before
/// Stop and Replace came, and a save belongs to an editor, which shows its own refusal. Exhaustive, so a new kind of
/// work decides.
fn prefix(work: Work) -> Option<&'static str> {
    match work {
        Work::Stop => Some("Stop: "),
        Work::Replace => Some("Replace: "),
        Work::Seen => Some("Seen: "),
        Work::Restart => Some("Restart: "),
        // A probe and a setup are host work, never on a session's key, so no row line ever names them; tunnel work
        // has a key of its own.
        Work::Rename
        | Work::Delete(_)
        | Work::Save
        | Work::Probe
        | Work::Setup
        | Work::TunnelOpen
        | Work::TunnelClose => None,
    }
}

/// What `key`'s row says about its operations now: the store's slot for it, a refusal or a success's notice (see the
/// module docs). `None` while it holds neither.
pub(crate) fn current_line(key: &str, cx: &App) -> Option<Failure> {
    session::current_operations(cx)
        .and_then(|operations| operations.read(cx).failure(&OpKey::session(key)).cloned())
}

// ---- The lines -------------------------------------------------------------------------------------------------------

/// `key`'s line under its row: [`current_line`] in `.action-error`'s 12-pixel red, or in the fill's ink on the selected
/// row (`.session-row.selected .action-error`). Named, as one label, by the operation and the escaped words.
pub(super) fn row_line(key: &str, selected: bool, cx: &App) -> Option<Div> {
    let line = current_line(key, cx)?;
    Some(line_text(
        SharedString::from(format!("session-outcome-{key}")),
        &line,
        refusal_ink(selected, theme::palette(cx)),
    ))
}

/// `key`'s line inside its open menu, which covers the row's own copy: the same words on the menu's own surface, so in
/// the ordinary red whatever the row's fill.
pub(super) fn menu_line(key: &str, cx: &App) -> Option<Div> {
    let line = current_line(key, cx)?;
    Some(line_text(
        SharedString::from(format!("session-menu-outcome-{key}")),
        &line,
        theme::palette(cx).danger,
    ))
}

/// `key`'s line in the open session's header, below its top row: the same words in the old header's error lines'
/// look (`.restart-error`, `.stop-error`, `.replace-error`: 12 pixels of `--danger`, padded 4 by 10 pixels).
pub(crate) fn pane_line(key: &str, cx: &App) -> Option<Div> {
    let line = current_line(key, cx)?;
    Some(
        div()
            .py(px(4.))
            .px(px(10.))
            .text_size(metrics::TEXT_SM)
            .text_color(theme::color(theme::palette(cx).danger))
            .child(sentence(
                SharedString::from(format!("session-header-outcome-{key}")),
                &line,
            )),
    )
}

/// The ink of the row's line: `.action-error`'s red, or the fill's ink on the selected row, where red on the accent
/// fill is hard to read.
pub(super) fn refusal_ink(selected: bool, palette: &theme::Palette) -> theme::Token {
    if selected {
        palette.on_fill
    } else {
        palette.danger
    }
}

/// `line` in `.action-error`'s look and `ink`, for the row and its menu alike: the operation's name as the app's words
/// and the host's words as their own element, so a direction override in them reorders nothing around them.
fn line_text(id: SharedString, line: &Failure, ink: theme::Token) -> Div {
    div()
        .pt(px(2.))
        .pb(px(8.))
        .px(px(14.))
        .text_size(metrics::TEXT_SM)
        .text_color(theme::color(ink))
        .child(sentence(id, line))
}

/// `line`'s words as one sentence named `id`: the operation's name, where it has one, as the app's words, then the
/// host's words as their own element.
fn sentence(id: SharedString, line: &Failure) -> HostSentence {
    let mut sentence = HostSentence::new(id);
    if let Some(prefix) = prefix(line.work) {
        sentence = sentence.text(prefix);
    }
    sentence.host(&line.message)
}

// ---- The long wait's line --------------------------------------------------------------------------------------------

/// How a long wait's line names `work` ("Replacing on buildbox…"), and the noun its element ids carry; `None` for work
/// that shows no wait. Exhaustive, so a new kind of work decides.
fn waiting(work: Work) -> Option<(&'static str, &'static str)> {
    match work {
        Work::Replace => Some(("Replacing", "replace")),
        Work::Restart => Some(("Restarting", "restart")),
        Work::Rename
        | Work::Delete(_)
        | Work::Save
        | Work::Stop
        | Work::Seen
        | Work::Probe
        | Work::Setup
        | Work::TunnelOpen
        | Work::TunnelClose => None,
    }
}

/// The line a Restart or Replace (`work`) of `row`, filed under `key`, shows once it has waited two seconds: the spinner
/// at `step`, "<Restarting|Replacing> on <host>…" with the host as the helm names it (escaped, as its own element;
/// "this host" when the helm names none, as in the old app), then the whole seconds, all in `ink`. The old
/// `.launch-progress`, at 12 pixels, the spinner in the text's color. `surface` prefixes its element ids
/// (`<surface>-<replace|restart>-progress-<key>`, and `…-progress-label-<key>` for the label), so the row's line and the
/// header's can be told apart while both show one Replace. No padding: the surface places it. `None` while there is
/// nothing to show.
///
/// `announce` makes the line a status that assistive technology announces, named by its label. Only one copy of a wait
/// may be: while the open session's Replace waits, its row and the header both show it, and the old row stayed silent
/// then (`announce: !selected`), leaving the header to say it.
#[allow(
    clippy::too_many_arguments,
    reason = "one line's inputs: where it is drawn, what it waits on, and how it is drawn"
)]
pub(crate) fn wait_line(
    surface: &str,
    work: Work,
    announce: bool,
    key: &str,
    row: &SessionRow,
    step: u8,
    ink: Hsla,
    cx: &App,
) -> Option<Stateful<Div>> {
    let (verb, noun) = waiting(work)?;
    let seconds = waited_seconds(key, work, cx)?;
    let mut label = HostSentence::new(SharedString::from(format!(
        "{surface}-{noun}-progress-label-{key}"
    )))
    .text(format!("{verb} on "));
    label = if row.host_name.is_empty() {
        label.text("this host")
    } else {
        label.host(&HostText::from_host(row.host_name.as_str()))
    };
    let label = label.text("…");
    let name = label.accessible_name();
    let selector = format!("{surface}-{noun}-progress");
    Some(
        div()
            .id(ElementId::Name(
                format!("{surface}-{noun}-progress-{key}").into(),
            ))
            .debug_selector(move || selector.clone())
            .when(announce, |line| line.role(Role::Status).aria_label(name))
            .flex()
            .flex_row()
            .items_center()
            .gap(px(6.))
            .min_w_0()
            .text_size(metrics::TEXT_SM)
            .text_color(ink)
            .child(delete::spinner_icon(step, px(12.), ink))
            .child(div().flex_1().min_w_0().child(label))
            .child(div().flex_none().child(format!("{seconds} s"))),
    )
}

// ---- What is kept ----------------------------------------------------------------------------------------------------

/// The wait clocks of one UI session, by session key. Each stops itself once the work it was started for is no longer in
/// flight on its session.
///
/// An entity rather than plain state so that every view that reads it while drawing (the row, later the header)
/// redraws when it changes, as views redraw for the operation store, with no observer of its own.
pub(crate) struct RowOutcomes {
    clocks: HashMap<String, Clock>,
}

/// One request's clock: the work it waits on (a Restart or a Replace), the whole seconds its line shows (`None` before
/// the first two have passed), and the deadline that moves it on. Dropping the clock drops the deadline, and with it the
/// chain.
struct Clock {
    work: Work,
    shown: Option<u64>,
    _next: ticker::Deadline,
}

/// Where the [`RowOutcomes`] is kept: a global because the list and, later, the header read the same one. It is not
/// keyed on the UI session (see the module docs, "One UI session").
#[derive(Default)]
struct Kept(Option<Entity<RowOutcomes>>);

impl Global for Kept {}

/// The [`RowOutcomes`], if one was made.
fn current(cx: &App) -> Option<Entity<RowOutcomes>> {
    cx.try_global::<Kept>()?.0.clone()
}

/// The [`RowOutcomes`], made when there is none.
fn current_or_new(cx: &mut App) -> Entity<RowOutcomes> {
    if let Some(outcomes) = current(cx) {
        return outcomes;
    }
    let outcomes = cx.new(|_| RowOutcomes {
        clocks: HashMap::new(),
    });
    cx.set_global(Kept(Some(outcomes.clone())));
    outcomes
}

impl RowOutcomes {
    /// `key`'s clock reached `seconds`: show them and arm the next step, unless its work has settled meanwhile (then
    /// the clock goes).
    fn tick(&mut self, key: String, seconds: u64, cx: &mut Context<Self>) {
        let waiting = self.clocks.get(&key).map(|clock| clock.work);
        if waiting.is_none() || in_flight(&key, cx) != waiting {
            self.clocks.remove(&key);
            cx.notify();
            return;
        }
        let next = step_at(
            cx.weak_entity(),
            key.clone(),
            PROGRESS_STEP,
            seconds + 1,
            cx,
        );
        if let Some(clock) = self.clocks.get_mut(&key) {
            clock.shown = Some(seconds);
            clock._next = next;
            cx.notify();
        }
    }
}

/// A deadline `delay` from now that moves `key`'s clock to `seconds`.
fn step_at(
    outcomes: WeakEntity<RowOutcomes>,
    key: String,
    delay: Duration,
    seconds: u64,
    cx: &mut App,
) -> ticker::Deadline {
    ticker::after(delay, cx, move |cx| {
        // Gone with its UI session; then there is nothing to move on.
        let _ = outcomes.update(cx, |outcomes, cx| outcomes.tick(key, seconds, cx));
    })
}

/// Start `key`'s clock for `work` (a Restart or a Replace), at the moment its request was sent: it shows two seconds
/// from now. A clock already running for the key (an earlier request whose settle has not been heard yet) starts over.
pub(crate) fn start_clock(key: &str, work: Work, cx: &mut App) {
    let outcomes = current_or_new(cx);
    let next = step_at(
        outcomes.downgrade(),
        key.to_owned(),
        PROGRESS_SHOWS_AFTER,
        PROGRESS_SHOWS_AFTER.as_secs(),
        cx,
    );
    outcomes.update(cx, |outcomes, cx| {
        outcomes.clocks.insert(
            key.to_owned(),
            Clock {
                work,
                shown: None,
                _next: next,
            },
        );
        cx.notify();
    });
}

/// The whole seconds `key`'s `work` has waited, once it has waited two; `None` before that, once it has settled, and for
/// any other work.
pub(crate) fn waited_seconds(key: &str, work: Work, cx: &App) -> Option<u64> {
    if in_flight(key, cx) != Some(work) {
        return None;
    }
    let outcomes = current(cx)?;
    let clock = outcomes.read(cx).clocks.get(key)?;
    (clock.work == work).then_some(clock.shown).flatten()
}

/// The work in flight on `key` in the current UI session's operation store.
fn in_flight(key: &str, cx: &App) -> Option<Work> {
    session::current_operations(cx)?
        .read(cx)
        .in_flight(&OpKey::session(key))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A refusal on the selected row was once drawn red on the accent fill, near unreadable; the old sheet switched it
    /// to the fill's ink (`.session-row.selected .action-error`). Specified, in both palettes: the fill's ink when
    /// selected, `--danger` otherwise.
    #[test]
    fn a_refusal_on_the_selected_row_takes_the_fills_ink() {
        for palette in [&theme::DARK, &theme::LIGHT] {
            assert_eq!(refusal_ink(true, palette), palette.on_fill);
            assert_eq!(refusal_ink(false, palette), palette.danger);
        }
    }

    /// The old app's error slot named the operation for Stop, Replace, Restart and a manual seen toggle ("Stop: …",
    /// "Replace: …", "Restart: …", "Seen: …"), and the ported Delete and Rename refusals have always shown the helm's
    /// words alone. Specified: Stop, Replace, Restart and Seen carry their name and a colon, the others nothing.
    #[test]
    fn only_stop_replace_restart_and_seen_lines_carry_their_name() {
        assert_eq!(prefix(Work::Stop), Some("Stop: "));
        assert_eq!(prefix(Work::Replace), Some("Replace: "));
        assert_eq!(prefix(Work::Seen), Some("Seen: "));
        assert_eq!(prefix(Work::Restart), Some("Restart: "));
        for work in [
            Work::Rename,
            Work::Save,
            Work::Delete(farhelm_proto::DeleteGuard::Unconditional),
        ] {
            assert_eq!(prefix(work), None, "{work:?}");
        }
    }
}
