//! A session's manual Mark read and Mark unread: the item in its row's menu, the click on its status dot, the request
//! both send, and what a refusal leaves on the row.
//!
//! SPEC.md, "Status" and "Session list". Ported from the old app's `list/row.rs` (`offers_mark_seen`, the item's label
//! and value, the dot's click through `StatusBadgeView` in `status.rs`), `list/view.rs` (`on_mark_seen`,
//! `seen_toggle_report`) and `api.rs` (`mark_seen`) at the reference commit. Here's the TLDR of the rules:
//!
//! - **Offered on a live row.** Running, waiting or idle, never on an ended or unclassified one, which has no dot and
//!   no seen state worth toggling. A row on an unreachable host still offers it, as in the old app: the helm keeps seen
//!   state itself, with no host to ask, so nothing there can refuse it. A retired containerized session offers neither,
//!   live or not: it supports Delete only ([`super::retired`]), and the old app fenced its seen writes the same way.
//!   A muted row gives that space to its own Unmute control and the helm records the seen stamp when the mute ends.
//! - **The label follows the row.** "Mark read" while the row has output nobody has looked at ([`status::unseen`]; a
//!   running or waiting row can have some too), which records the row's latest activity as seen, and "Mark unread"
//!   otherwise, which clears the stamp ([`Toggle`]). The menu and the dot read it on every draw, so an open menu
//!   relabels itself when output arrives; a click sends what the label it was drawn with said.
//! - **One request, through the operation store,** under the session's key ([`Work::Seen`], which says what that
//!   costs the session's other actions). Nothing is shown in advance: the dot changes when the helm pushes the row with
//!   its new stamp, which it does before it replies.
//! - **A refusal shows on the row** under the action's name, until the session's next operation starts
//!   ([`super::outcome`]). Unlike a failed automatic mark, which is only logged (SPEC.md, "Errors and diagnostics"), the
//!   user asked for this one, so losing it silently would look like it worked.
//! - **The item closes the menu; the dot is a pointer shortcut** ([`MarkToggle`] says how it keeps its click from the
//!   row), named by its action and its session, since every live row has one. The menu item is the keyboard path.
//! - **A manual Mark unread of the open session sticks** until the user opens another session and comes back, or new
//!   output arrives. Nothing here arranges that; the automatic mark's memory does ([`super::selection`]).

use farhelm_helm_client::{ApiPath, Method, PathError};
use farhelm_proto::SessionSeenRequest;
use farhelm_proto::ui_channel::SessionRow;
use gpui::{AnyElement, App, IntoElement as _, SharedString};
use serde_json::json;

use super::delete;
use super::row_marks::HelpPlace;
use super::selection::Listed;
use super::status::{self, Live};
use crate::components::{HoverHelp, MarkToggle, Role};
use crate::host_text::{HostSentence, HostText};
use crate::operations::{Command, Work};
use crate::theme;

/// The id prefix of a row's status dot as a toggle; the rest is the session's key. Deliberately not under the row's own
/// prefix (`session-row-`), which tests and the bridge use to find the rows' open controls.
pub(crate) const DOT_ID_PREFIX: &str = "session-seen-";

/// What a manual toggle of a row's seen state does, as the row reads now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Toggle {
    /// The row has unseen output: record this activity stamp, the row's latest, as seen.
    Read(i64),
    /// The row reads seen: clear its stamp, so it reads unseen until it is marked again.
    Unread,
}

impl Toggle {
    /// The action's name: the menu item's label and accessible name, and the start of the dot's accessible name.
    pub(crate) fn label(self) -> &'static str {
        match self {
            Toggle::Read(_) => "Mark read",
            Toggle::Unread => "Mark unread",
        }
    }

    /// The line the menu item shows under its label (the old item's description).
    pub(crate) fn description(self) -> &'static str {
        match self {
            Toggle::Read(_) => "Treat the latest output as seen",
            Toggle::Unread => "Show new output again",
        }
    }

    /// The menu item's hover help, worded like its neighbors' ("Rename: give this session a new title").
    pub(crate) fn help(self) -> &'static str {
        match self {
            Toggle::Read(_) => "Mark read: treat the latest output as seen",
            Toggle::Unread => "Mark unread: show new output again",
        }
    }

    /// Keep the action's name through a refusal, including when the bulk action cannot start.
    pub(crate) fn work(self) -> Work {
        let read = matches!(self, Self::Read(_));
        Work::Seen { read }
    }

    /// The route's body for this toggle: the stamp to record, or `null` to clear it.
    pub(crate) fn request(self) -> SessionSeenRequest {
        SessionSeenRequest {
            seen_activity_at: match self {
                Toggle::Read(activity) => Some(activity),
                Toggle::Unread => None,
            },
        }
    }
}

/// The toggle belongs to a live session. Mute owns the mark and its click
/// until the helm ends it.
pub(crate) fn offered(row: &SessionRow) -> Option<Toggle> {
    if !row.info.status.is_live() || status::muted(row) || super::retired::is(row) {
        return None;
    }
    Some(if status::unseen(row) {
        Toggle::Read(row.info.effective_activity())
    } else {
        Toggle::Unread
    })
}

/// The dot's hover help: the status word, then the action, as the old dot's (`Idle — new output — Mark read`). A dot
/// that only named its action would hide what the mark means.
pub(crate) fn dot_help(live: Live, toggle: Toggle) -> String {
    format!("{} — {}", live.word(), toggle.label())
}

/// The session's manual seen toggle, `PUT /api/sessions/{id}/seen` with the route's own request type.
pub(crate) fn command(session: &str, toggle: Toggle) -> Result<Command, PathError> {
    Ok(Command::new(
        toggle.work(),
        Method::Put,
        ApiPath::api(&["sessions", session, "seen"])?,
        // An optional integer always encodes, so `json!` (which panics on a failed encode) cannot fail here.
        Some(json!(toggle.request())),
        "PUT /api/sessions/[session]/seen",
    ))
}

/// Send `key`'s `toggle` through the operation store ([`super::menu::send`]). A refusal is the store's to keep and the
/// row's to show; a success needs nothing, since the helm has pushed the new stamp by the time it replies.
pub(crate) fn send(key: &str, toggle: Toggle, cx: &mut App) {
    super::menu::send(key, command(key, toggle), cx);
}

/// `session`'s live status mark as drawn in its row, at pulse step `step`: the dot, as a [`MarkToggle`] that sends the
/// row's toggle when clicked. Its accessible name is the action and the session's title ("Mark read: <title>", the title
/// as host text), so a screen reader's list of buttons tells one row's dot from another's; its hover help names the
/// status and the action, beside the sidebar as every row hover is ([`HelpPlace::Row`]). Its ring is square on the
/// dot's 8-pixel box, as the old outline was (`.status-dot.tooltip-target-ring`, on a box with no radius of its own): a
/// ring of the dot's own round size would lie on the shape's edge and vanish into it, where a square's corners stand
/// clear of every shape. While the session has a request in flight the click does nothing.
pub(super) fn dot(session: Listed, live: Live, selected: bool, step: u64, cx: &App) -> AnyElement {
    let row = session.row;
    let mark = live.mark(theme::palette(cx), selected, row.stale, step);
    // The connected waiting dot can remain visible under a mute. Its
    // Unmute control is beside the title instead.
    let Some(toggle) = offered(row) else {
        return mark.into_any_element();
    };
    let id = SharedString::from(format!("{DOT_ID_PREFIX}{}", session.key));
    let name = HostSentence::new(id.clone())
        .text(format!("{}: ", toggle.label()))
        .host(&HostText::from_host(row.info.title.as_str()))
        .accessible_name();
    let toggle_control = MarkToggle::new(
        id,
        name,
        HelpPlace::Row { selected }.place(HoverHelp::tooltip(dot_help(live, toggle))),
        Role::Button,
    )
    .mark(mark);
    if delete::busy(session.key, cx) {
        return toggle_control.into_any_element();
    }
    let key = session.key.to_owned();
    toggle_control
        .on_click(move |_, _, cx| send(&key, toggle, cx))
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The toggle's wording and value are the old item's, and the value decides whether a click records or clears the
    /// seen stamp, so a mix-up would mark read what the label said it would mark unread. Specified: unseen output reads
    /// "Mark read" with its description and help and sends the row's latest activity; a seen row reads "Mark unread"
    /// and sends `null`; an ended row offers nothing; the command is `Work::Seen`, a PUT on the session's seen route.
    #[test]
    fn the_toggle_follows_the_seen_state_and_sends_the_stamp_or_null() {
        let row = |status: serde_json::Value, seen: Option<i64>| -> SessionRow {
            let mut entry = crate::test_support::session_entry("a").value;
            entry["status"] = status;
            entry["last_activity_at"] = json!(250);
            entry["seen_activity_at"] = json!(seen);
            serde_json::from_value(entry).unwrap()
        };
        let idle = json!({"state": "idle"});
        let unseen = offered(&row(idle.clone(), Some(100))).unwrap();
        assert_eq!(unseen, Toggle::Read(250));
        assert_eq!(
            (unseen.label(), unseen.description(), unseen.help()),
            (
                "Mark read",
                "Treat the latest output as seen",
                "Mark read: treat the latest output as seen"
            )
        );
        assert_eq!(
            serde_json::to_value(unseen.request()).unwrap(),
            json!({"seen_activity_at": 250})
        );
        let seen = offered(&row(idle, Some(250))).unwrap();
        assert_eq!(seen, Toggle::Unread);
        assert_eq!(
            (seen.label(), seen.description(), seen.help()),
            (
                "Mark unread",
                "Show new output again",
                "Mark unread: show new output again"
            )
        );
        assert_eq!(
            serde_json::to_value(seen.request()).unwrap(),
            json!({"seen_activity_at": null})
        );
        assert_eq!(
            offered(&row(json!({"state": "running"}), None)),
            Some(Toggle::Read(250)),
            "a running row with output nobody looked at is unseen too"
        );
        assert_eq!(
            offered(&row(json!({"state": "exited", "exit_code": 0}), None)),
            None
        );
        let built = command("a", Toggle::Unread).unwrap();
        assert_eq!(built.work(), Work::Seen { read: false });
        assert_eq!(built.path().as_str(), "/api/sessions/a/seen");
    }
}
