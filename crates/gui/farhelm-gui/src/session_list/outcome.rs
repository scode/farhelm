//! What a session's row says about its own operations beyond "in flight": the refusal of the last one, the notice a
//! successful one left, and how long a Replace has been waiting.
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
//!   refusal style, as the old app's one error slot drew them. A Stop or Replace line carries its operation's name in
//!   front of the words, as that slot did, and a Rename or Delete refusal shows the words alone, as before.
//! - **The Replace clock is deadlines, never a timer of its own.** Nothing shows for the first two seconds; then the
//!   line shows the whole seconds since the request started, moved on once a second by a chain of
//!   [`ticker::after`] deadlines, which fire whether or not the window is active (a wait is about the user's time,
//!   not about drawing; SPEC_impl.md, "Animation time: the ticker"). Each step checks that the session's Replace is
//!   still in flight, so the line hides the moment the Replace settles and the clock itself goes at its next step.
//! - **One UI session.** A clock shows only while the current operation store has its Replace in flight, and every
//!   step checks the same, so after a reset (SPEC_impl.md, "One failure path: the reset") a clock kept from before
//!   shows nothing and goes at its next step: the new store has none of the old Replaces in flight, and a Replace
//!   started under it starts that key's clock over ([`start_clock`]). So the clocks need no keying on the store.
//!
//! The words and lines are functions of a session's key, so the open session's header can show the same ones when it
//! is ported.

use std::collections::HashMap;
use std::time::Duration;

use gpui::{
    App, AppContext as _, Context, Div, Entity, Global, ParentElement, SharedString, Styled,
    WeakEntity, div, px,
};

use super::replace;
use crate::host_text::HostSentence;
use crate::operations::{Failure, OpKey, Work};
use crate::theme::{self, metrics};
use crate::{session, ticker};

/// How long a Replace waits before its row says so: the old `LaunchProgress`'s two whole seconds, so a quick reply
/// adds nothing to the row.
pub(crate) const PROGRESS_SHOWS_AFTER: Duration = Duration::from_secs(2);

/// How often the Replace clock moves on once it shows.
const PROGRESS_STEP: Duration = Duration::from_secs(1);

// ---- The words -------------------------------------------------------------------------------------------------------

/// The name a line about `work` carries in front of the host's words on the row, as the old app's error slot did:
/// "Stop: " and "Replace: ", for a refusal and a notice alike. A Rename or a Delete refusal shows the words alone, as it
/// did before Stop and Replace came, and a save belongs to an editor, which shows its own refusal. Exhaustive, so a new
/// kind of work decides.
fn prefix(work: Work) -> Option<&'static str> {
    match work {
        Work::Stop => Some("Stop: "),
        Work::Replace => Some("Replace: "),
        Work::Rename | Work::Delete(_) | Work::Save => None,
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
    let mut sentence = HostSentence::new(id);
    if let Some(prefix) = prefix(line.work) {
        sentence = sentence.text(prefix);
    }
    div()
        .pt(px(2.))
        .pb(px(8.))
        .px(px(14.))
        .text_size(metrics::TEXT_SM)
        .text_color(theme::color(ink))
        .child(sentence.host(&line.message))
}

// ---- What is kept ----------------------------------------------------------------------------------------------------

/// The Replace clocks of one UI session, by session key. Each stops itself once its Replace is no longer in flight.
///
/// An entity rather than plain state so that every view that reads it while drawing (the row, later the header)
/// redraws when it changes, as views redraw for the operation store, with no observer of its own.
pub(crate) struct RowOutcomes {
    clocks: HashMap<String, Clock>,
}

/// One Replace's clock: the whole seconds its row shows (`None` before the first two have passed), and the deadline
/// that moves it on. Dropping the clock drops the deadline, and with it the chain.
struct Clock {
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
    /// `key`'s clock reached `seconds`: show them and arm the next step, unless its Replace has settled meanwhile (then
    /// the clock goes).
    fn tick(&mut self, key: String, seconds: u64, cx: &mut Context<Self>) {
        if !replace::in_flight(&key, cx) {
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

/// Start `key`'s Replace clock, at the moment its request was sent: it shows two seconds from now. A clock already
/// running for the key (an earlier Replace whose settle has not been heard yet) starts over.
pub(crate) fn start_clock(key: &str, cx: &mut App) {
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
                shown: None,
                _next: next,
            },
        );
        cx.notify();
    });
}

/// The whole seconds `key`'s Replace has waited, once it has waited two; `None` before that and once it has settled.
pub(crate) fn replace_seconds(key: &str, cx: &App) -> Option<u64> {
    if !replace::in_flight(key, cx) {
        return None;
    }
    current(cx)?.read(cx).clocks.get(key)?.shown
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

    /// The old app's error slot named the operation for Stop and Replace ("Stop: …", "Replace: …"), and the ported
    /// Delete and Rename refusals have always shown the helm's words alone. Specified: Stop and Replace carry their name
    /// and a colon, the others nothing.
    #[test]
    fn only_stop_and_replace_lines_carry_their_name() {
        assert_eq!(prefix(Work::Stop), Some("Stop: "));
        assert_eq!(prefix(Work::Replace), Some("Replace: "));
        for work in [
            Work::Rename,
            Work::Save,
            Work::Delete(farhelm_proto::DeleteGuard::Unconditional),
        ] {
            assert_eq!(prefix(work), None, "{work:?}");
        }
    }
}
