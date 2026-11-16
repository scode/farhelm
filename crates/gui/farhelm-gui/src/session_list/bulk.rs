//! Work on the marked sessions as a group ([`super::marking`]). So far that is one thing: the line a marked row shows
//! when a bulk action skipped it because its own commands were refused at that moment.
//!
//! SPEC.md, "Session list": bulk metadata choices "reuse the ordinary writes and row-local errors", and "each refused
//! row gets the ordinary action-specific not-started message, including a row engaged by its own question or rename".
//! The ordinary sends swallow a start the operation store refuses (the row menu never sends one, since it draws the
//! item unavailable), so a bulk action checks the row first, sends nothing for it, and records its line here:
//! "<action>: not started, " and the operation store's shared reason ([`NotStarted::BUSY_REASON`]), the lead-in naming
//! what did not happen as the store's reason asks.
//!
//! **One line per session, until its next operation starts.** The lines live beside the operation store's own slot for
//! the session and go the way it does: when the session's next operation starts, or with the UI session (a reset).
//! Every surface that draws the session's outcome line (the row, its menu, and the open session's header and
//! interrupted card) draws them through [`super::outcome`], beside the store's slot, so a session has one drawing path
//! for what its operations say. The store gives no word when an operation starts, so the list prunes the lines whenever
//! the store changes ([`prune`], from `SessionList::settle`): a line recorded while the session's request was in flight
//! waits for that request to settle, and goes once another is in flight.
//!
//! NOTE: A request that settles and is followed at once by another on the same session, within one round of the store's
//! notifications, looks to [`prune`] like the first one still in flight, so the line stays until that next request
//! settles and a third starts.
//!
//! NOTE: The bulk Stop and Delete batches (one request at a time per host, the bulk question, the selection held back
//! for a bulk Delete) come here with their work; until then nothing else lives in this module.

use std::collections::HashMap;

use gpui::{App, AppContext as _, Context, Entity, EntityId, Global, SharedString};

use super::delete::busy;
use crate::operations::NotStarted;
use crate::session;

/// The not-started lines of one UI session, by session key. An entity rather than plain state so that every view that
/// draws a line (the list's rows, the open session's pane) redraws when one comes or goes, as views redraw for the
/// operation store, with no observer of its own (`super::replace_yolo`'s questions are kept the same way).
pub(crate) struct NotStartedLines {
    /// The UI session the lines were recorded under: a reset makes a new one, and the lines of the old one mean nothing
    /// after it, so a line is read only while this is still the current UI session.
    session: Option<EntityId>,
    lines: HashMap<String, Line>,
}

/// One session's line, and whether its session has been free of requests since it was recorded.
struct Line {
    words: SharedString,
    /// False while the request that refused the bulk action is still in flight; once it has settled, the next one to
    /// start ends the line.
    freed: bool,
}

/// Where the [`NotStartedLines`] is kept: a global, because the list and the open session's pane read the same one.
#[derive(Default)]
struct Kept(Option<Entity<NotStartedLines>>);

impl Global for Kept {}

/// The current UI session's id, which keys the lines to their UI session; `None` while the window is inert.
fn ui_session(cx: &App) -> Option<EntityId> {
    session::connection(cx).and_then(|connection| {
        connection
            .read(cx)
            .session()
            .map(|session| session.entity_id())
    })
}

/// Record that the bulk action labelled `label` ("Pin", "Mark read") was not started on `key`'s row, replacing any line
/// it had. Nothing is recorded while the window is inert.
pub(super) fn record_not_started(key: &str, label: &str, cx: &mut App) {
    let Some(now) = ui_session(cx) else {
        return;
    };
    let freed = !busy(key, cx);
    let lines = match cx.try_global::<Kept>().and_then(|kept| kept.0.clone()) {
        Some(lines) => lines,
        None => {
            let lines = cx.new(|_| NotStartedLines {
                session: None,
                lines: HashMap::new(),
            });
            cx.set_global(Kept(Some(lines.clone())));
            lines
        }
    };
    lines.update(cx, |lines, cx| {
        if lines.session != Some(now) {
            lines.session = Some(now);
            lines.lines.clear();
        }
        lines.lines.insert(
            key.to_owned(),
            Line {
                words: format!("{label}: not started, {}", NotStarted::BUSY_REASON).into(),
                freed,
            },
        );
        cx.notify();
    });
}

/// `key`'s not-started line in the current UI session, if it has one.
pub(crate) fn not_started_line(key: &str, cx: &App) -> Option<SharedString> {
    let lines = cx.try_global::<Kept>()?.0.as_ref()?.read(cx);
    if lines.session != ui_session(cx) {
        return None;
    }
    lines.lines.get(key).map(|line| line.words.clone())
}

/// Drop every line whose session's next operation has started, and every line of a UI session that is no longer
/// current. Called whenever the operation store changes (see the module docs).
pub(super) fn prune(cx: &mut App) {
    let Some(lines) = cx.try_global::<Kept>().and_then(|kept| kept.0.clone()) else {
        return;
    };
    let now = ui_session(cx);
    lines.update(cx, |lines, cx: &mut Context<NotStartedLines>| {
        let before = lines.lines.len();
        if lines.session != now {
            lines.lines.clear();
        }
        lines.lines.retain(|key, line| {
            let in_flight = busy(key, cx);
            if !line.freed {
                line.freed = !in_flight;
                return true;
            }
            !in_flight
        });
        if lines.lines.len() != before {
            cx.notify();
        }
    });
}
