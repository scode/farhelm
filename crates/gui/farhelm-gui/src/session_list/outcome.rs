//! What a session says about its own operations beyond "in flight": the refusal of the last one, the notice a
//! successful one left, and how long a Restart or a Replace has been waiting, on its row and in the open session's
//! header.
//!
//! SPEC.md, "Lifecycle operations": "A failure or partial result appears on the source session's row", and "After about
//! two seconds waiting for the reply, the sidebar, session header and interrupted-session card share a spinner, the
//! host and elapsed seconds, updated about once a second". Ported from the old app's one error slot per session
//! (`errors` in `session_ops.rs`, filled by `list/view.rs` with "Stop: …" and "Replace: …") and its `LaunchProgress`
//! (`launch_progress.rs`) at the reference commit. A Delete's notice, and the one a Replace's source's Delete carried,
//! are not this module's: they go to the list's Delete notice band (`super::delete_notice`), since the row they were
//! about is gone. Here's the TLDR of the rules:
//!
//! - **One line per session, shown until the session's next operation starts.** This is the one statement of that rule;
//!   the row, its menu and each operation's module link here. The line is the operation store's slot for the session
//!   ([`crate::operations::Operations::failure`]): a refusal, or a partial result a success carried (a Stop that could
//!   not close a tab), which the operation's completion records there with [`crate::operations::Operations::note`]. The
//!   store keeps either until the key's next start or a reset, whoever starts that work, so an editor or a later header
//!   needs no call here. Both are drawn alike, in the refusal style, as the old app's one error slot drew them. A Stop,
//!   Replace, Restart, seen-toggle, pin or move line carries its operation's name in front of the words, as that slot
//!   did, and a Rename or Delete refusal shows the words alone, as before. The open session's header shows the same
//!   line below its top row ([`pane_line`]), as the old header's error lines did.
//! - **A bulk action's not-started line is one of them.** A marked row a bulk action skipped because its commands were
//!   refused says so ("Pin: not started, …", "Delete: not started, …"), and [`super::bulk`] records that line in the
//!   same slot with [`crate::operations::Operations::note`], so it lasts until the session's next operation starts and a
//!   refusal the busy request settles with replaces it, as the old app's one error slot did.
//! - **The wait clock is the ticker's, never a timer of its own.** One clock serves Replace and Restart, the two
//!   requests SPEC.md gives this line, and a session has at most one, since it has at most one request in flight. A
//!   [`ticker::after`] deadline shows the line two seconds after the request; from then the line counts the whole
//!   seconds since it, read from [`ticker::wall_now`], and the ticker's shared one-second clock
//!   ([`ticker::every_second`]) moves it on. Both fire whether or not the window is active (SPEC_impl.md, "Animation
//!   time: the ticker"). The threshold is a deadline of its own because the shared clock's cadence would show the line
//!   up to a second late. The line hides the moment its request settles; the clock goes at its threshold or at the
//!   next second, whichever comes first.
//! - **One UI session.** A clock shows only while the current operation store has its work in flight, and every prune
//!   checks the same, so after a reset (SPEC_impl.md, "One failure path: the reset") a clock kept from before shows
//!   nothing and goes at the next prune: the new store has none of the old requests in flight, and a request started
//!   under it starts that key's clock over ([`start_clock`]). So the clocks need no keying on the store.
//!
//! The words and lines are functions of a session's key, so the row and the open session's header show the same ones.

use std::collections::HashMap;
use std::time::{Duration, SystemTime};

use farhelm_proto::ui_channel::SessionRow;
use gpui::{
    App, AppContext as _, Context, Div, ElementId, Entity, Global, Hsla, InteractiveElement,
    ParentElement, Role, SharedString, Stateful, StatefulInteractiveElement as _, Styled,
    Subscription, div, prelude::FluentBuilder as _, px,
};

use crate::components::spinner_icon;
use crate::host_text::{HostSentence, HostText};
use crate::operations::{Failure, OpKey, Work};
use crate::theme::{self, metrics};
use crate::{session, ticker};

/// How long a Restart or a Replace waits before its line shows: the old `LaunchProgress`'s two whole seconds, so a quick
/// reply adds nothing.
pub(crate) const PROGRESS_SHOWS_AFTER: Duration = Duration::from_secs(2);

// ---- The words -------------------------------------------------------------------------------------------------------

/// The name a line about `work` carries in front of the host's words on the row, as the old app's error slot did:
/// "Stop: ", "Replace: " and "Restart: ", for a refusal and a notice alike, "Seen: " for a refused Mark read or Mark
/// unread (the old app's `seen_toggle_report`), and "Pin: " and "Move pin: " for a refused pin and move. A Rename or a
/// Delete refusal (an environment Delete's included) shows the words alone, as it did before Stop and Replace came, and
/// a save belongs to an editor, which shows its own refusal. Exhaustive, so a new kind of work decides.
pub(super) fn prefix(work: Work) -> Option<&'static str> {
    match work {
        Work::Stop => Some("Stop: "),
        Work::Replace => Some("Replace: "),
        Work::Seen => Some("Seen: "),
        Work::Restart => Some("Restart: "),
        // The old app's tab error line ("Open tab: " and the supervisor's words: a vanished folder, a shell that died
        // at once). A new tab is the session's own work, so its refusal shows where the session's others do.
        Work::TabOpen => Some("Open tab: "),
        // The old app's pin lines (`on_pin` and `on_move_pin` in `list/view.rs`): a pin's and a move's refusals are
        // told apart by their names.
        Work::Pin => Some("Pin: "),
        Work::MovePin => Some("Move pin: "),
        // The old list's error slot for an environment's Suspend and Resume ("Sandbox: " for either provider, as the
        // old `HeaderAction::Sandbox` wrote it), and Stop's for Stop and suspend, which the old list ran as a Stop
        // with its suspend flag set.
        Work::Suspend | Work::Resume => Some("Sandbox: "),
        Work::StopAndSuspend => Some("Stop: "),
        // A probe, a setup, a host's removal, Update and uninstall are host work, never on a session's key, so no row
        // line ever names them; tunnel work has a key of its own, a launch is the launcher's, on a launch key, and an
        // approval answer is on its request's key, whose card shows its refusal. A tab's close is on the tab's key,
        // whose refusal the tab strip shows.
        Work::Rename
        | Work::Delete(_)
        | Work::EnvironmentDelete
        | Work::Save
        | Work::Probe
        | Work::Setup
        | Work::TunnelOpen
        | Work::TunnelClose
        | Work::Launch
        | Work::RemoveHost
        | Work::SendFeedback
        | Work::Answer
        | Work::UpdateHost
        | Work::UninstallHost
        | Work::RetryHost
        | Work::AdoptHost
        | Work::TrashDelete
        | Work::TabClose => None,
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
        | Work::TunnelClose
        | Work::Launch
        | Work::RemoveHost
        | Work::SendFeedback
        | Work::Answer
        | Work::UpdateHost
        | Work::UninstallHost
        | Work::RetryHost
        | Work::AdoptHost
        | Work::TrashDelete
        | Work::TabOpen
        | Work::TabClose
        | Work::Pin
        | Work::MovePin
        | Work::Suspend
        | Work::Resume
        | Work::StopAndSuspend
        | Work::EnvironmentDelete => None,
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
            .child(spinner_icon(step, px(12.), ink))
            .child(div().flex_1().min_w_0().child(label))
            .child(div().flex_none().child(format!("{seconds} s"))),
    )
}

// ---- What is kept ----------------------------------------------------------------------------------------------------

/// The wait clocks of one UI session, by session key, and the one-second clock that moves their lines on. Each clock
/// goes once the work it was started for is no longer in flight on its session.
///
/// An entity rather than plain state so that every view that reads it while drawing (the row, the header) redraws
/// when it changes, as views redraw for the operation store, with no observer of its own; the one-second clock's
/// notifications reach those views the same way.
///
/// It holds the one-second clock itself, though the foundation ownership doc says a view holds one: GPUI redraws
/// every window that read this entity when the clock notifies it. The notification also runs [`RowOutcomes::prune`],
/// which lets a settled request's clock go, something neither of the two views drawing these lines could do alone.
pub(crate) struct RowOutcomes {
    clocks: HashMap<String, Clock>,
    /// The ticker's one-second clock, held exactly while some clock's line shows (its threshold passed and its work
    /// still in flight), so the app is woken once a second only while some line has a count to move on.
    second: Option<ticker::Clock>,
    /// Prunes the clocks on every notification of this entity, the one-second clock's included: nothing else calls
    /// back here once a line shows, so this is where a settled request's clock goes.
    _prune: Subscription,
}

/// One request's clock: the work it waits on (a Restart or a Replace), when it was sent by the ticker's wall clock,
/// and the deadline that shows its line two seconds later, which is `None` once it has fired. Dropping the clock drops
/// the deadline.
struct Clock {
    work: Work,
    sent: SystemTime,
    threshold: Option<ticker::Deadline>,
}

impl Clock {
    /// Whether the line shows: its threshold has passed.
    fn shows(&self) -> bool {
        self.threshold.is_none()
    }
}

/// Where the [`RowOutcomes`] is kept: a global because the list and the header read the same one. It is not keyed on
/// the UI session (see the module docs, "One UI session").
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
    let outcomes = cx.new(|cx| RowOutcomes {
        clocks: HashMap::new(),
        second: None,
        _prune: cx.observe_self(RowOutcomes::prune),
    });
    cx.set_global(Kept(Some(outcomes.clone())));
    outcomes
}

impl RowOutcomes {
    /// `key`'s threshold passed: its line shows from now, and the one-second clock moves it on. A clock whose work has
    /// settled meanwhile goes instead, with no second clock taken for it.
    fn threshold_passed(&mut self, key: &str, cx: &mut Context<Self>) {
        if let Some(clock) = self.clocks.get_mut(key) {
            clock.threshold = None;
        }
        // The notification runs the prune (`_prune`), which drops a settled clock or takes the one-second clock.
        cx.notify();
    }

    /// Drop every clock whose work is no longer in flight on its session (settled, or gone with a reset's store), and
    /// hold the one-second clock exactly while a remaining line shows. Changes nothing a view draws, since a line is
    /// drawn only while its work is in flight, so it notifies nobody.
    fn prune(&mut self, cx: &mut Context<Self>) {
        self.clocks
            .retain(|key, clock| in_flight(key, cx) == Some(clock.work));
        self.hold_second(cx);
    }

    /// Take the one-second clock while some clock's line shows, and let go of it once none does.
    fn hold_second(&mut self, cx: &mut Context<Self>) {
        let counting = self.clocks.values().any(Clock::shows);
        if counting != self.second.is_some() {
            self.second = counting.then(|| ticker::every_second(cx));
        }
    }
}

/// Start `key`'s clock for `work` (a Restart or a Replace), at the moment its request was sent: it shows two seconds
/// from now. A clock already running for the key (an earlier request whose settle has not been heard yet) starts over.
pub(crate) fn start_clock(key: &str, work: Work, cx: &mut App) {
    let outcomes = current_or_new(cx);
    let weak = outcomes.downgrade();
    let shown = key.to_owned();
    let threshold = ticker::after(PROGRESS_SHOWS_AFTER, cx, move |cx| {
        // Gone with its UI session; then there is nothing to show.
        let _ = weak.update(cx, |outcomes, cx| outcomes.threshold_passed(&shown, cx));
    });
    let sent = ticker::wall_now(cx);
    outcomes.update(cx, |outcomes, cx| {
        outcomes.clocks.insert(
            key.to_owned(),
            Clock {
                work,
                sent,
                threshold: Some(threshold),
            },
        );
        cx.notify();
    });
}

/// The whole seconds `key`'s `work` has waited, once it has waited two; `None` before that, once it has settled, and for
/// any other work. Read from the wall clock, and never less than the two seconds the line waits for, so a wall clock
/// that a slewing or a reset puts behind the deadline's timer never shows "1 s" or "0 s".
pub(crate) fn waited_seconds(key: &str, work: Work, cx: &App) -> Option<u64> {
    if in_flight(key, cx) != Some(work) {
        return None;
    }
    let outcomes = current(cx)?;
    let clock = outcomes.read(cx).clocks.get(key)?;
    if clock.work != work || !clock.shows() {
        return None;
    }
    let waited = ticker::wall_now(cx)
        .duration_since(clock.sent)
        .unwrap_or_default();
    Some(waited.max(PROGRESS_SHOWS_AFTER).as_secs())
}

/// Whether the kept clocks hold the ticker's one-second clock, and how many request clocks they keep: what the tests
/// read to show that a settled request lets go of the one-second clock. `None` before any clock was started.
#[cfg(test)]
pub(crate) fn kept(cx: &App) -> Option<(bool, usize)> {
    current(cx).map(|outcomes| {
        let outcomes = outcomes.read(cx);
        (outcomes.second.is_some(), outcomes.clocks.len())
    })
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

    /// The old app's error slot named the operation for Stop, Replace, Restart, a manual seen toggle, a pin and a move
    /// ("Stop: …", "Replace: …", "Restart: …", "Seen: …", "Pin: …", "Move pin: …"), and the ported Delete and Rename
    /// refusals have always shown the helm's words alone. Specified: those six carry their name and a colon, the others
    /// nothing.
    #[test]
    fn only_stop_replace_restart_seen_and_pin_lines_carry_their_name() {
        assert_eq!(prefix(Work::Stop), Some("Stop: "));
        assert_eq!(prefix(Work::Replace), Some("Replace: "));
        assert_eq!(prefix(Work::Seen), Some("Seen: "));
        assert_eq!(prefix(Work::Restart), Some("Restart: "));
        assert_eq!(prefix(Work::Pin), Some("Pin: "));
        assert_eq!(prefix(Work::MovePin), Some("Move pin: "));
        for work in [
            Work::Rename,
            Work::Save,
            Work::Delete(farhelm_proto::DeleteGuard::Unconditional),
        ] {
            assert_eq!(prefix(work), None, "{work:?}");
        }
    }
}
