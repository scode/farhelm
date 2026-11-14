//! One session's row: its status mark and title, its ended status when it has one, and where it runs.
//!
//! Ported from the old app's `list/row.rs` (the row body) and the stylesheet's `.session-row` rules at the reference
//! commit. SPEC.md, "Session list": a row's first line shows its status, locality mark and title in aligned columns; a
//! noncompact row shows an ended status in full on a line of its own that wraps; the last line names the host and the
//! working directory as one string, `host:~/path`, starting under the title. The selected row is a solid accent fill
//! with everything on it in the inverse colors.
//!
//! Everything a host wrote (the title, the host's name, the directory, a stop annotation) is host text: escaped, and
//! each value its own text element, so a direction override in one cannot reorder anything around it.
//!
//! NOTE: The locality column is kept empty, at its width, so titles start where they did in the old app; the locality,
//! agent and permission marks, the activity age, the stale cause, the pin and the notification bell are not ported yet.

use farhelm_helm_client::Collections;
use farhelm_proto::ui_channel::{HostView, SessionRow};
use gpui::{
    AnyElement, App, FontWeight, InteractiveElement, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement, Styled, Window, div, prelude::FluentBuilder as _, px, relative,
};

use super::selection::Listed;
use super::status::{self, Badge};
use crate::components::{HoverHelp, ListRow, Role};
use crate::host_text::{self, HostSentence, HostText};
use crate::theme::{self, Palette, metrics};

/// The id prefix of a session row's open control; the rest is the session's key, so the control's focus and hover
/// follow the session when the list reorders.
pub(crate) const ROW_ID_PREFIX: &str = "session-row-";

/// The id prefix of a session's list item, the row around the open control; the rest is the session's key.
pub(crate) const ITEM_ID_PREFIX: &str = "session-item-";

/// The indent of the lines under the first: the status and locality columns and their gaps (`.session-row-meta`).
fn under_title() -> gpui::Pixels {
    (metrics::ROW_SLOT + metrics::ROW_SLOT_GAP) * 2.
}

/// Draw `session`'s row, its status mark at the list's pulse step `pulse_step` (which only a pulsing mark uses).
/// `on_open` runs when the user activates it.
pub(crate) fn row(
    session: Listed,
    collections: &Collections,
    selected: bool,
    pulse_step: u64,
    on_open: impl Fn(&mut Window, &mut App) + 'static,
    cx: &App,
) -> AnyElement {
    let palette = theme::palette(cx);
    let row = session.row;
    let id: SharedString = format!("{ROW_ID_PREFIX}{}", session.key).into();
    let title = HostText::from_host(row.info.title.as_str());
    let badge = status::badge(row, format!("{id}-status").into());
    // The row's accessible name is its title, escaped as the screen shows it; its status word joins the hover help,
    // which is also its accessible description.
    let label = HostSentence::new(id.clone()).host(&title).accessible_name();
    let item_label = label.clone();
    let help = match Badge::word(badge.as_ref()) {
        Some(word) => format!("{word} — open this session"),
        None => "Open this session".to_owned(),
    };

    let (live, ended) = match badge {
        Some(Badge::Live(live)) => (Some(live), None),
        Some(Badge::Ended { kind, line }) => (None, Some((kind, line))),
        None => (None, None),
    };
    let title_color = if selected {
        palette.on_fill
    } else {
        palette.fg_0
    };
    let identity = div()
        .flex()
        .flex_row()
        .items_center()
        .gap(metrics::ROW_SLOT_GAP)
        .min_w_0()
        .child(
            div()
                .w(metrics::ROW_SLOT)
                .flex_none()
                .flex()
                .items_center()
                .justify_center()
                .children(live.map(|live| live.mark(palette, selected, row.stale, pulse_step))),
        )
        .child(div().w(metrics::ROW_SLOT).flex_none())
        .child(
            host_text::element(&title)
                .flex_1()
                .min_w_0()
                .truncate()
                .text_size(metrics::TEXT_ROW_TITLE)
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme::color(title_color)),
        );

    let mut control = ListRow::new(id, label, HoverHelp::tooltip(help), Role::Button)
        .selected(selected)
        .line(identity);
    if let Some((kind, line)) = ended {
        control = control.line(
            div()
                .pl(under_title())
                .child(status::ended_line(kind, line, palette, selected)),
        );
    }
    control = control
        .line(meta_line(session, collections, palette, selected))
        .on_click(move |_, window, cx| on_open(window, cx));

    // The row itself is the list's item, and it is what carries "selected" for assistive technology: AccessKit
    // reports selection only on item-like roles (a list item, an option), not on a button. The old app marked its open
    // button `aria-current` instead, which GPUI cannot express. The item is named like its button, by the title, so
    // nothing reads out its element id instead.
    div()
        .id(SharedString::from(format!(
            "{ITEM_ID_PREFIX}{}",
            session.key
        )))
        .role(Role::ListItem)
        .aria_label(item_label)
        .aria_selected(selected)
        .w_full()
        .border_b_1()
        .border_color(theme::color(palette.border_row))
        .when(selected, |row| row.bg(theme::color(palette.accent_fill)))
        .child(control)
        .into_any_element()
}

/// The `host:~/path` line: the host's short name (or the helm's name for a host the registry no longer lists), a
/// colon, and the working directory with the user's home folded to `~`, cut from the left when it does not fit so the
/// end of the path stays visible.
fn meta_line(
    session: Listed,
    collections: &Collections,
    palette: &Palette,
    selected: bool,
) -> impl IntoElement {
    let row = session.row;
    let host = host_label(row, collections.get::<HostView>(&HostView::key(row.host)));
    let cwd = HostText::from_host(abbreviate_home(&row.info.cwd));
    let color = if selected {
        palette.on_accent_dim
    } else {
        palette.fg_1
    };
    div()
        .flex()
        .flex_row()
        .min_w_0()
        .pl(under_title())
        .text_size(metrics::TEXT_SM)
        .line_height(px(16.))
        .text_color(theme::color(color))
        // No host name at all (no registry row and none from the helm): the directory alone, never a dangling
        // separator.
        .when(!host.is_empty(), |line| {
            let host = HostText::from_host(host);
            line.child(
                host_text::element(&host)
                    .flex_none()
                    .max_w(relative(0.4))
                    .truncate(),
            )
            .child(div().flex_none().child(":"))
        })
        .child(
            host_text::element(&cwd)
                .flex_1()
                .min_w_0()
                .overflow_hidden()
                .whitespace_nowrap()
                .text_ellipsis_start(),
        )
}

/// The host part of a row's `host:~/path` line, as the old app named it: the host's short name when the registry lists
/// the host (its alias, `Local`, or the name derived from its destination), else the registry's own name for a host
/// with none of those (a sandbox or container host has no destination), else the name the helm sent with the row. Empty
/// only when all of those are, and the line then shows the directory alone.
pub(crate) fn host_label(row: &SessionRow, host: Option<&HostView>) -> String {
    host.map(|host| {
        let short = farhelm_proto::host_name::host_short_name(
            host.is_local(),
            host.destination.as_deref(),
            host.alias.as_deref(),
        );
        if short.is_empty() {
            host.name.clone()
        } else {
            short
        }
    })
    .filter(|name| !name.is_empty())
    .unwrap_or_else(|| row.host_name.clone())
}

/// `cwd` with a personal home folded to `~` (`/home/alice/src` becomes `~/src`), or unchanged. A path operation on the
/// host's own bytes; the caller escapes the result.
///
/// Ported from the old app's `abbreviate_home`, with its three exclusions: a `.` or `..` segment anywhere (the path
/// would not resolve under that home, so folding it would claim something the path contradicts), macOS's
/// `/Users/Shared` (not anyone's home), and a bare account with nothing after it (folding would erase the one thing the
/// path says). The row only shortens; the full path is what a later summary card shows.
pub(crate) fn abbreviate_home(cwd: &str) -> String {
    for prefix in ["/home/", "/Users/"] {
        let Some(rest) = cwd.strip_prefix(prefix) else {
            continue;
        };
        if rest
            .split('/')
            .any(|segment| segment == "." || segment == "..")
        {
            continue;
        }
        let Some(slash) = rest.find('/') else {
            continue;
        };
        let (user, remainder) = rest.split_at(slash);
        // An empty account (`/home//x`) names no home; a one-byte remainder is the bare account's trailing slash.
        if user.is_empty() || remainder.len() <= 1 {
            continue;
        }
        if prefix == "/Users/" && user == "Shared" {
            continue;
        }
        return format!("~{remainder}");
    }
    cwd.to_owned()
}
