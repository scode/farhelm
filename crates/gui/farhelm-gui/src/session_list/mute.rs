//! The row and marked-session menus send the same mute operation. The
//! duration becomes one absolute deadline at the click, in the user's zone.

use std::rc::Rc;
use std::time::{Duration, UNIX_EPOCH};

use farhelm_helm_client::{ApiPath, Method, PathError};
use farhelm_proto::ui_channel::SessionRow;
use farhelm_proto::{SessionMuteRequest, SessionStatus};
use gpui::{AnyElement, App, IntoElement, SharedString, Styled, Window, svg};

use super::row::Steps;
use super::row_marks::HelpPlace;
use super::selection::Listed;
use super::status::{self, Live};
use crate::assets::Icon;
use crate::components::{HoverHelp, MarkToggle, MenuItem, Role};
use crate::host_text::{HostSentence, HostText};
use crate::local_time::{self, Zone};
use crate::operations::{Command, Work};
use crate::theme;
use crate::ticker;

/// The bell is an outline glyph, so it needs the ended marks' 10 px (in the same 12 px slot) to read as a bell;
/// at the filled dot's 8 px its strokes blur together.
const BELL_SIZE: gpui::Pixels = gpui::px(10.);

/// The choices offered by either Mute submenu, before it knows the click time.
#[derive(Clone, Copy)]
enum MuteFor {
    Hour,
    Morning,
    Indefinite,
}

impl MuteFor {
    const ALL: [Self; 3] = [Self::Hour, Self::Morning, Self::Indefinite];

    fn item(self) -> (&'static str, &'static str) {
        match self {
            Self::Hour => ("hour", "For 1 hour"),
            Self::Morning => ("morning", "Until tomorrow morning"),
            Self::Indefinite => ("indefinite", "Until I unmute"),
        }
    }

    /// Compute the end from the activation's wall clock. A bulk action
    /// passes this same request to all its admitted sessions.
    fn request(self, now: Duration, zone: Zone) -> SessionMuteRequest {
        let millis = i64::try_from(now.as_millis()).unwrap_or(i64::MAX);
        let seconds = i64::try_from(now.as_secs()).unwrap_or(i64::MAX);
        let until = match self {
            Self::Hour => Some(millis.saturating_add(3_600_000)),
            Self::Morning => Some(local_time::next_morning(seconds, zone).saturating_mul(1_000)),
            Self::Indefinite => None,
        };
        SessionMuteRequest::Mute { until }
    }
}

/// The same three choices appear in either menu, each saying when it would end if chosen now (so "Until tomorrow
/// morning" opened at 02:00 says it ends at 06:00 today). The request itself is still computed from the clock at
/// the click, so a menu left open across 06:00 never stores yesterday's morning.
pub(super) fn durations(
    prefix: &str,
    apply: impl Fn(SessionMuteRequest, &mut Window, &mut App) + 'static,
    cx: &App,
) -> Vec<MenuItem> {
    let apply = Rc::new(apply);
    let opened = ticker::wall_now(cx)
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let zone = local_time::zone(cx);
    let now = i64::try_from(opened.as_secs()).ok();
    MuteFor::ALL
        .into_iter()
        .map(|duration| {
            let (id, label) = duration.item();
            let apply = apply.clone();
            let ends = match duration.request(opened, zone) {
                SessionMuteRequest::Mute { until: Some(until) } => {
                    format!("Ends {}", local_time::mute_end(until, now, zone))
                }
                _ => "Ends when you unmute it".to_owned(),
            };
            MenuItem::new(
                format!("{prefix}-{id}"),
                Icon::BellOff,
                label,
                HoverHelp::tooltip(ends.clone()),
                Role::MenuItem,
            )
            .description(ends)
            .on_click(move |_, window, cx| {
                let now = ticker::wall_now(cx)
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default();
                apply(duration.request(now, local_time::zone(cx)), window, cx);
            })
        })
        .collect()
}

/// Both menus and the bell use the ordinary per-session operation. No
/// local mute is drawn before the helm pushes its stored choice.
pub(super) fn send(key: &str, request: SessionMuteRequest, cx: &mut App) {
    super::menu::send(key, command(key, request), cx);
}

fn command(key: &str, request: SessionMuteRequest) -> Result<Command, PathError> {
    Ok(Command::new(
        Work::Mute,
        Method::Put,
        ApiPath::api(&["sessions", key, "mute"])?,
        Some(serde_json::json!(request)),
        "PUT /api/sessions/[session]/mute",
    ))
}

/// A real, connected wait gets its red dot and a bell after its title. A blocked session is a wait like a detected
/// question ([`SessionRow::effectively_waiting`]), so it breaks through a mute the same way.
pub(super) fn after_title(row: &SessionRow) -> bool {
    status::muted(row) && !row.stale && row.effectively_waiting()
}

/// Give other muted rows the bell in the dot slot, including an unclassified
/// row or a stale last-known wait. Ended rows keep their ordinary shape.
pub(super) fn replaces_dot(row: &SessionRow) -> bool {
    status::muted(row)
        && (row.info.status.is_live() || row.info.status == SessionStatus::Unknown)
        && !after_title(row)
}

/// The shared words used by the bell and the row's compact and full help.
/// The zone is the viewer's, even if another device set the deadline.
pub(super) fn label(row: &SessionRow, now: Option<i64>, zone: Zone) -> Option<String> {
    row.mute
        .map(|mute| local_time::mute_label(mute.until, now, zone))
}

/// One Unmute control in either of its placements. Preserve the real status
/// in help because the bell itself cannot distinguish Running from Idle.
pub(super) fn bell(session: Listed, selected: bool, steps: Steps, cx: &App) -> AnyElement {
    let row = session.row;
    let palette = theme::palette(cx);
    let ink = if selected {
        palette.on_fill
    } else if row.stale {
        palette.fg_2
    } else {
        palette.fg_1
    };
    let status = status::live(row).map_or("Unclassified", Live::word);
    let muted = label(row, steps.now, steps.zone).unwrap_or_else(|| "Muted".to_string());
    let help = format!("{status} — {muted} — Unmute");
    let id = SharedString::from(format!("session-mute-bell-{}", session.key));
    let name = HostSentence::new(id.clone())
        .text(format!("{help}: "))
        .host(&HostText::from_host(row.info.title.as_str()))
        .accessible_name();
    let mark = MarkToggle::new(
        id,
        name,
        HelpPlace::Row { selected }.place(HoverHelp::tooltip(help)),
        Role::Button,
    )
    .mark(
        svg()
            .path(Icon::BellOff.path())
            .size(BELL_SIZE)
            .flex_none()
            .text_color(theme::color(ink)),
    );
    if super::retired::is(row) || super::menu::refused(session.key, cx) {
        return mark.into_any_element();
    }
    let key = session.key.to_owned();
    mark.on_click(move |_, _, cx| send(&key, SessionMuteRequest::Unmute, cx))
        .into_any_element()
}
