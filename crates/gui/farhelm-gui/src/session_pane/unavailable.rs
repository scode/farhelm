//! The unavailable card: what the session pane shows while the open session's host cannot be reached, with the band
//! above it that keeps the session's last-known status and age.
//!
//! SPEC.md, "Session list": "A session on an unreachable host shows one calm accent card naming the cause, with the
//! helm's remedy and smaller diagnostics beneath it. Its metadata — title, directory, last-known status — remains
//! available, with last-known status and age in exactly one band. A terminal already open keeps its last screen and tab
//! strip dimmed and inert beneath the card, clearly no longer live. A session opened while unreachable has no terminal
//! or tab strip." Ported from the old app's `SessionUnavailableCard` and `.stale-metadata` (`session_view.rs`,
//! `hosts.rs`'s `stale_session_notice` and `app.css` at the reference commit). Here's the TLDR of the rules:
//!
//! - **When.** While the store holds the open session's row as stale: the helm marks every row of a host it is not
//!   connected to that way ([`shows_row`]). Stale beats interrupted: a host that cannot be asked anything cannot be
//!   restarted either, so an interrupted session there shows this card, never the interrupted one (`card.rs`).
//! - **Over the terminal it had, or over nothing.** The pane keeps the terminals it already shows when their session's
//!   host drops away, and makes the whole set inert
//!   ([`TerminalSet::set_inert`](crate::terminal::set::TerminalSet::set_inert)), tabs and strip included, which draws
//!   the veil this card stands on; the card draws none of its own over a terminal. A session opened while its host is
//!   away gets no terminals and no strip until the host answers, when the pane mounts them as for any other opening
//!   (`session_pane.rs`); meanwhile the card draws the terminal's ground and veil under itself, as the old overlay's
//!   scrim lay over the empty terminal area. The terminals' own notices are held back while the card shows, as the old
//!   stylesheet hid the terminal's banner and reconnect surface under it: "Session unavailable — reconnecting" under a
//!   card that already says why would only say it twice.
//! - **What it says** ([`Notice`]): the heading names the cause in the session list's own words (`status::cause`), so
//!   the row and the card cannot disagree on it; under it the explanation, which is the host's remedy where waiting
//!   will not help and otherwise says that the host is not answering and that the view reattaches by itself; then the
//!   smaller diagnostics, the host's evidence line from the Hosts panel. Both come from the host's phase lines
//!   (`hosts::identity::phase`), so the card and the host's row say the same thing.
//! - **Focus.** The card is home while it shows, so focus never rests on a terminal that takes nothing: an inert
//!   terminal holds no focus (`AttachedTerminal::set_inert`), so when the host drops away while one of the open
//!   session's terminals holds focus, or a layer closes that would give focus back to one, the focus coordinator sends
//!   it home to the card; once the card goes, focus goes home to the terminal again. The old app left nothing focused
//!   under its card (the browser blurred the inert terminal); here the card holds focus, a recorded change. Focus
//!   elsewhere (a header button, a copy field in the details bar) stays where it is either way. The card is a keyboard
//!   scroll stop that Tab reaches, named "Session unavailable", so a remedy that runs past a short pane can be read
//!   without a pointer, as the old card's region was (`components::scroll_stop_on`, on a handle the pane keeps so it
//!   can be home before its first frame).
//! - **The band.** The details bar leaves a stale session's status and age out (`details.rs`); they are drawn here
//!   instead, between the top row's lines and the terminal area, the status as the list's mark grey and still, since a
//!   cached status is no evidence of anything live.
//!
//! - **A suspended or stopped host offers Resume** ([`Offer`]): "Resume", the filled primary button, on a session's own
//!   environment (`session_list::environment`), and "Resume host" on an ordinary host, the old card's, while the host
//!   is suspended or stopped and not waiting for its place. It sends the same Resume as the row's and the header's,
//!   with no question, on the session's key; a refusal shows on the card under the explanation rather than above it. A 504 at the helm's bound is not one: the card then follows the
//!   pushed phase ("is resuming").
//! - **A suspended environment says so in its own words.** Its heading is "Session suspended" with the pause glyph in
//!   place of the spinner, its explanation the provider's (what is preserved and what still bills), and its
//!   diagnostics who suspended it and when, with a failed resume's reason first ([`ProviderUi::suspended_detail`]). An
//!   environment that is starting again names that cause ("Session unavailable — starting environment") with the
//!   provider's starting words. A stopped container keeps the ordinary card: it is never called suspended.
//!
//! NOTE: Not ported: the old card's fixed, full-height layout for a short window. Here the card's region scrolls within
//! the area the terminal takes, which at the window's minimum height is still several hundred pixels tall.

use farhelm_helm_client::Collections;
use farhelm_proto::tensorlake::LifecycleDirection;
use farhelm_proto::ui_channel::{Availability, Collection, HostStateView, HostView, SessionRow};
use gpui::{
    AnyElement, App, Div, FocusHandle, FontWeight, InteractiveElement as _, IntoElement,
    ParentElement, Role, ScrollHandle, SharedString, Stateful, StatefulInteractiveElement as _,
    Styled, Window, div, prelude::FluentBuilder as _, px,
};

use super::details::{self, Status};
use crate::assets::Icon;
use crate::components::{self, Button, HoverHelp, Ring, Tier, spinner_icon};
use crate::environment_ui::ProviderUi;
use crate::host_text::{HostSentence, HostText};
use crate::hosts::identity::{self, phase};
use crate::local_time::Zone;
use crate::operations::Work;
use crate::session;
use crate::session_list::age::{self, Age};
use crate::session_list::environment::{self, Environment};
use crate::session_list::status::{self, HostLookup};
use crate::session_list::{menu, outcome};
use crate::theme::{self, Palette, metrics};
use crate::{focus, layers};

/// The card's keyboard stop, the region over the terminal area, by which tests and the bridge find it.
pub(crate) const CARD_ID: &str = "session-unavailable";
/// The card itself, inside the stop's scrolling region.
pub(crate) const INNER_CARD_ID: &str = "session-unavailable-card";
/// The heading's id.
pub(crate) const HEADING_ID: &str = "session-unavailable-heading";
/// The explanation's id.
pub(crate) const EXPLANATION_ID: &str = "session-unavailable-explanation";
/// The diagnostics line's id.
pub(crate) const DETAIL_ID: &str = "session-unavailable-detail";
/// The band's id, and its parts'.
pub(crate) const BAND_ID: &str = "session-stale-band";
pub(crate) const BAND_STATUS_ID: &str = "session-stale-status";
pub(crate) const BAND_AGE_ID: &str = "session-stale-age";

/// `.session-unavailable-overlay`'s `padding: 20px`.
const PAD: f32 = 20.;
/// `.session-unavailable-card`'s `max-width: 42rem`.
const MAX_WIDTH: f32 = 672.;
/// `.session-unavailable-card`'s padding, vertical then horizontal.
const CARD_PAD: (f32, f32) = (14., 20.);
/// `.session-unavailable-heading`: its gap, text size and spinner.
const HEADING_GAP: f32 = 10.;
const HEADING_SIZE: f32 = 15.;
const SPINNER: f32 = 16.;
/// `.session-unavailable-explanation`'s and `-detail`'s `margin-top`, and the detail's smaller text.
const EXPLANATION_GAP: f32 = 9.;
const DETAIL_GAP: f32 = 7.;
const DETAIL_SIZE: f32 = 11.5;
/// `.stale-metadata`: its gap, and its padding at the sides and below (none above).
const BAND_GAP: f32 = 8.;
const BAND_PAD: (f32, f32) = (10., 6.);

// ---- When the card shows --------------------------------------------------------------------------------------------

/// Whether `row` takes the unavailable card: the helm cannot reach its host (see the module docs, "When").
pub(crate) fn shows_row(row: &SessionRow) -> bool {
    row.stale
}

/// [`shows_row`] for the row the current UI session's store holds under `key`; `false` while it holds none.
pub(crate) fn shows(key: &str, cx: &App) -> bool {
    session::current_store(cx).is_some_and(|store| {
        store
            .read(cx)
            .collections()
            .get::<SessionRow>(key)
            .is_some_and(shows_row)
    })
}

// ---- What the card says ---------------------------------------------------------------------------------------------

/// The card's words for one stale session: the cause its heading names, the explanation under it and the smaller
/// diagnostics line, if any, and the Resume it offers. The old `SessionUnavailableNotice`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Notice {
    /// The session list's cause, as its row names it (`status::cause`): "Waiting for supervisor", say; or the card's
    /// own for a session's environment ("Suspended", "Starting environment", the old card's).
    pub(crate) cause: &'static str,
    pub(crate) explanation: HostSentence,
    pub(crate) detail: Option<HostSentence>,
    /// Whether the heading reads "Session suspended" with the pause glyph: a session's own environment, suspended.
    pub(crate) suspended: bool,
    /// The Resume the card offers, if any (see the module docs).
    pub(crate) resume: Option<Offer>,
}

/// A card's Resume: the host it resumes, and the provider's words when that host is the session's own environment
/// (`None` for an ordinary host, whose button reads "Resume host").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Offer {
    pub(crate) host: i64,
    pub(crate) environment: Option<ProviderUi>,
    /// Stopped rather than suspended, so the environment's Resume starts it again ([`ProviderUi::resume_help`]).
    pub(crate) stopped: bool,
}

/// The id of the card's Resume button.
pub(crate) const RESUME_ID: &str = "session-unavailable-resume";

/// The id of the line under the explanation that shows a refused Resume.
pub(crate) const RESUME_FAILURE_ID: &str = "session-unavailable-resume-failure";

impl Notice {
    /// The notice for `row`, whose host the registry in `collections` places (or fails to).
    pub(crate) fn of(row: &SessionRow, collections: &Collections, zone: Zone) -> Notice {
        let failure = match collections
            .meta(Collection::Hosts)
            .map(|meta| &meta.availability)
        {
            Some(Availability::Unavailable { error }) => Some(error.as_str()),
            Some(Availability::Available) | None => None,
        };
        // A duplicate's lines name the entry that holds the same machine as its own row shows it.
        let twin = |id: i64| {
            collections
                .get::<HostView>(&HostView::key(id))
                .map(identity::display_name)
        };
        notice(
            &row.host_name,
            HostLookup::of(collections, row.host),
            failure,
            twin,
            zone,
        )
    }

    /// The heading: the cause continues the sentence, so it is lowered (the old card's `to_ascii_lowercase`; every
    /// cause is the app's own ASCII wording, never a host's). A suspended environment's reads "Session suspended".
    pub(crate) fn heading(&self) -> String {
        if self.suspended {
            return "Session suspended".to_owned();
        }
        format!("Session unavailable — {}", self.cause.to_ascii_lowercase())
    }
}

/// The notice for a session on the host the helm calls `host_name`, which the registry places as `lookup`; `failure`
/// is the helm's words for a host list it could not read, and `twin` names another host by id, as a duplicate's lines
/// need; `zone` is the viewer's, for the times the host's evidence gives. The old `stale_session_notice`, case for case.
/// A session that names no host is "This session's host" ("this session's host" inside a sentence), and the name is the
/// helm's as sent, as the old card's was.
pub(crate) fn notice(
    host_name: &str,
    lookup: HostLookup<'_>,
    failure: Option<&str>,
    twin: impl Fn(i64) -> Option<HostText>,
    zone: Zone,
) -> Notice {
    let cause = status::cause(lookup);
    let named = |line: HostSentence, inside: bool| {
        if host_name.trim().is_empty() {
            line.text(if inside {
                "this session's host"
            } else {
                "This session's host"
            })
        } else {
            line.host(&HostText::from_host(host_name))
        }
    };
    let explanation = HostSentence::new(EXPLANATION_ID);
    let detail = HostSentence::new(DETAIL_ID);
    // The card's evidence line after the host's name. A lifecycle request names the provider as Hosts does.
    let evidence_line = || named(detail.clone(), false).text(": ");
    let evidence = |host: &HostView| phase::evidence(evidence_line(), &host.state, &twin, zone);
    let (explanation, detail) = match lookup {
        HostLookup::Pending => (named(explanation.text("Checking "), true).text("…"), None),
        HostLookup::Failed => (
            explanation.text("The helm's host state could not be refreshed."),
            failure.map(|error| detail.host(&HostText::from_host(error))),
        ),
        HostLookup::Absent => (
            explanation.text(
                "This host is no longer registered with this helm. Re-adding the destination would reach it again.",
            ),
            Some(named(detail, false)),
        ),
        HostLookup::Known(host) => match &host.state {
            // The host answers again and the helm's listing is about to catch up.
            HostStateView::Connected { .. } => {
                (explanation.text("Refreshing this session's state."), None)
            }
            // Paused on purpose: the session waits there for it, rather than this view retrying.
            HostStateView::Suspended { .. } => (
                named(explanation, false)
                    .text(" is suspended; this session continues when it resumes."),
                Some(evidence(host)),
            ),
            HostStateView::Stopped { .. } => {
                (phase::evidence(explanation, &host.state, &twin, zone), None)
            }
            HostStateView::TensorlakeChanging { direction, .. } => (
                named(explanation, false).text(match direction {
                    LifecycleDirection::Suspending => {
                        " is suspending; this session continues when the host resumes."
                    }
                    LifecycleDirection::Resuming => {
                        " is resuming; this view reconnects when the host is ready."
                    }
                }),
                Some(phase::host_evidence(evidence_line(), host, &twin, zone)),
            ),
            HostStateView::Connecting { .. }
            | HostStateView::Unreachable { .. }
            | HostStateView::VersionSkew { .. }
            | HostStateView::IdentityMismatch { .. }
            | HostStateView::OwnedElsewhere { .. }
            | HostStateView::IdentityUnverified { .. }
            | HostStateView::Duplicate { .. }
            | HostStateView::Retired { .. }
            | HostStateView::Deleting { .. } => (
                phase::remedy(explanation.clone(), &host.state, &twin).unwrap_or_else(|| {
                    named(explanation, false).text(
                        " is not answering. The session keeps running there; this view reattaches by itself.",
                    )
                }),
                Some(evidence(host)),
            ),
        },
    };
    let notice = Notice {
        cause,
        explanation,
        detail,
        suspended: false,
        resume: None,
    };
    match lookup {
        HostLookup::Known(host) => environment_notice(notice, host, zone),
        HostLookup::Pending | HostLookup::Failed | HostLookup::Absent => notice,
    }
}

/// `notice` with what a suspended, stopped or starting `host` changes in it: the Resume it offers, and, for a session's
/// own environment, the provider's words for being suspended or starting again (the old `sandbox_sessions::
/// unavailable_notice` and `stale_session_notice`'s `resume_host`).
fn environment_notice(mut notice: Notice, host: &HostView, zone: Zone) -> Notice {
    let environment = Environment::of_host(host).map(|environment| environment.ui);
    notice.resume = match host.state {
        HostStateView::Suspended { .. } => Some(Offer {
            host: host.id,
            environment,
            stopped: false,
        }),
        HostStateView::Stopped {
            waiting_for_place: false,
            ..
        } => Some(Offer {
            host: host.id,
            environment,
            stopped: true,
        }),
        _ => None,
    };
    let Some(ui) = environment else {
        return notice;
    };
    match &host.state {
        HostStateView::Suspended {
            observed_at,
            suspended_by_farhelm_at,
            resume_failed,
        } => Notice {
            cause: "Suspended",
            explanation: HostSentence::new(EXPLANATION_ID).text(ui.suspended_explanation()),
            detail: Some(ui.suspended_detail(
                HostSentence::new(DETAIL_ID),
                *observed_at,
                *suspended_by_farhelm_at,
                resume_failed.as_deref(),
                zone,
            )),
            suspended: true,
            ..notice
        },
        HostStateView::TensorlakeChanging { starting: true, .. } => Notice {
            cause: "Starting environment",
            explanation: HostSentence::new(EXPLANATION_ID).text(ui.starting_help()),
            ..notice
        },
        _ => notice,
    }
}

// ---- Drawing the card -----------------------------------------------------------------------------------------------

/// What the card draws: its notice, the focus handle its stop tracks (home while it shows), the scroll state of its
/// region, both kept by the pane, and the spinner's step.
pub(crate) struct Drawn<'a> {
    /// The session the card is for, whose key its Resume is sent on.
    pub(crate) key: &'a str,
    pub(crate) notice: Notice,
    pub(crate) focus: &'a FocusHandle,
    pub(crate) scroll: &'a ScrollHandle,
    pub(crate) step: u8,
    /// Whether no terminal is drawn under the card (a session opened while its host was away), so the card draws the
    /// old area's terminal ground and veil itself; over a terminal, the inert terminal draws its own veil.
    pub(crate) alone: bool,
}

/// The card for `drawn`, laid over the whole area the terminals take: the old `.session-unavailable-overlay` region,
/// which scrolls, with the card centered in it while it fits and at its top once it does not, so the heading and the
/// last line are both reachable. Visible to the crate for the screenshot goldens.
///
/// The card itself is the old `.session-unavailable-card`: on `--bg-2` with a one-pixel accent edge and the menu
/// shadow, at most 42rem wide, the heading in the accent beside the turning spinner, the explanation in the quiet ink
/// and the diagnostics quieter and smaller still. It names itself by its heading as a status, as the old card's
/// `role="status"` was read.
pub(crate) fn draw(drawn: Drawn, window: &mut Window, cx: &mut App) -> AnyElement {
    let Drawn {
        key,
        notice,
        focus,
        scroll,
        step,
        alone,
    } = drawn;
    let palette = theme::palette(cx);
    let accent = theme::color(palette.accent);
    let heading = notice.heading();
    // A suspended environment's heading carries the pause glyph, still, where any other cause turns the spinner.
    let glyph = if notice.suspended {
        gpui::svg()
            .path(Icon::Pause.path())
            .size(px(SPINNER))
            .flex_none()
            .text_color(accent)
            .into_any_element()
    } else {
        spinner_icon(step, px(SPINNER), accent).into_any_element()
    };
    let resume = notice.resume.map(|offer| resume_button(key, offer, cx));
    // A refused Resume shows here, the card being where it was pressed; the header leaves it out meanwhile.
    let refused = outcome::current_line(key, cx)
        .filter(|failure| failure.work == Work::Resume && notice.resume.is_some())
        .map(|failure| {
            div()
                .mt(px(DETAIL_GAP))
                .text_size(px(DETAIL_SIZE))
                .text_color(theme::color(palette.fg_1))
                .child(HostSentence::new(RESUME_FAILURE_ID).host(&failure.message))
        });
    let card = div()
        .id(INNER_CARD_ID)
        .debug_selector(|| INNER_CARD_ID.into())
        .flex_none()
        .w_full()
        .max_w(px(MAX_WIDTH))
        .mx_auto()
        .my_auto()
        .py(px(CARD_PAD.0))
        .px(px(CARD_PAD.1))
        .bg(theme::color(palette.bg_2))
        .border_1()
        .border_color(accent)
        .rounded(metrics::RADIUS_MD)
        .shadow(layers::menu_shadow(palette))
        .role(Role::Status)
        .aria_label(SharedString::from(heading.clone()))
        .child(
            div()
                .id(HEADING_ID)
                .debug_selector(|| HEADING_ID.into())
                .role(Role::Heading)
                .aria_label(SharedString::from(heading.clone()))
                .flex()
                .flex_row()
                .items_center()
                .gap(px(HEADING_GAP))
                .text_color(accent)
                .text_size(px(HEADING_SIZE))
                .font_weight(FontWeight::SEMIBOLD)
                .child(glyph)
                .child(heading),
        )
        .child(
            div()
                .mt(px(EXPLANATION_GAP))
                .text_size(metrics::TEXT_SM)
                .text_color(theme::color(palette.fg_1))
                .child(notice.explanation),
        )
        .children(resume.map(|button| div().mt(px(EXPLANATION_GAP)).flex().child(button)))
        .children(refused)
        .children(notice.detail.map(|detail| {
            div()
                .mt(px(DETAIL_GAP))
                .text_size(px(DETAIL_SIZE))
                .text_color(theme::color(palette.fg_2))
                .child(detail)
        }));
    let scroller = div()
        .id("session-unavailable-scroller")
        .size_full()
        .overflow_y_scroll()
        .p(px(PAD))
        .flex()
        .flex_col()
        .child(card);
    // The stop fills the area: `focus_stop` makes the stop the positioned box its ring is drawn in, so the stop cannot
    // be the absolutely placed layer itself, and sits in one that is.
    let stop = div()
        .id(CARD_ID)
        .debug_selector(|| CARD_ID.into())
        .size_full()
        .child(focus::scroll_region(scroller, scroll));
    // A two-pixel ring along the region's own edge, drawn inside it, as the old region's outline was.
    let ring = Ring {
        inset_x: px(0.),
        inset_y: px(0.),
        radius: px(0.),
    };
    let stop = components::scroll_stop_on(
        stop,
        &SharedString::from(CARD_ID),
        focus,
        scroll,
        Role::Region,
        "Session unavailable".into(),
        ring,
        window,
        cx,
    );
    // Alone, the old empty `.terminal-panes` showed through the overlay's scrim: the terminal's ground under the veil,
    // which is the look the inert terminal gives when there is one.
    div()
        .absolute()
        .top_0()
        .left_0()
        .right_0()
        .bottom_0()
        .when(alone, |layer| {
            layer.bg(theme::color(theme::TERMINAL.background)).child(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .right_0()
                    .bottom_0()
                    .bg(theme::color(palette.scrim)),
            )
        })
        .child(stop)
        .into_any_element()
}

/// The card's Resume for `key`'s session, as `offer` says: "Resume" in the primary tier for its own environment, with
/// the provider's help, or "Resume host" for an ordinary host (the old card's plain button). It sends the session's
/// Resume (`session_list::environment::resume`), and is unavailable while the session's commands are refused, a Resume
/// already pending among them.
fn resume_button(key: &str, offer: Offer, cx: &App) -> Button {
    let (label, tier, help) = match offer.environment {
        Some(ui) => ("Resume", Tier::Primary, ui.resume_help(offer.stopped)),
        None => (
            "Resume host",
            Tier::Ghost,
            "Resume host: this session continues when its host resumes",
        ),
    };
    let key = key.to_owned();
    Button::new(RESUME_ID, label, HoverHelp::tooltip(help), Role::Button)
        .icon(Icon::Play)
        .tier(tier)
        .compact()
        .disabled(menu::refused(&key, cx))
        .on_click(move |_, _, cx| environment::resume(&key, offer.host, cx))
}

/// Whether `key`'s card shows a refused Resume itself (see [`draw`]), so the lines above the terminal leave it out
/// rather than show it twice.
pub(crate) fn shows_resume_refusal(key: &str, cx: &App) -> bool {
    let offered = session::current_store(cx).is_some_and(|store| {
        let collections = store.read(cx).collections();
        collections
            .get::<SessionRow>(key)
            .filter(|row| shows_row(row))
            .is_some_and(|row| {
                Notice::of(row, collections, crate::local_time::zone(cx))
                    .resume
                    .is_some()
            })
    });
    offered && outcome::current_line(key, cx).is_some_and(|failure| failure.work == Work::Resume)
}

// ---- The band -------------------------------------------------------------------------------------------------------

/// What the band shows for a stale session: its last-known status as the details bar draws a status (a live mark, or
/// an ended status's words) and its age, each when there is one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Band {
    pub(crate) status: Option<Status>,
    pub(crate) age: Option<Age>,
}

impl Band {
    /// The band for `row` with the viewer's clock at `now` ([`age::seconds`]) and the age's stamp in `zone`: `None` for
    /// a session the card does not show, and for one with no status to draw (nobody classified it) and no age, for
    /// which the old view drew no band either.
    pub(crate) fn of(row: &SessionRow, now: Option<i64>, zone: Zone) -> Option<Band> {
        let band = Band {
            status: status::live(row)
                .map(Status::Live)
                .or_else(|| status::ended_words(row, false).map(Status::Ended)),
            age: age::of(row.info.effective_activity(), now, zone),
        };
        let something = band.status.is_some() || band.age.is_some();
        (shows_row(row) && something).then_some(band)
    }

    /// The band: the status, then the age, on the raised chrome between the top row's lines and the terminal area,
    /// each drawn by the details bar's own functions ([`details::badge`], [`details::age_part`]), so the band and the
    /// bar cannot draw a status or an age differently; a live mark as a stale row's, grey and still.
    pub(crate) fn draw(self, palette: &Palette) -> Stateful<Div> {
        div()
            .id(BAND_ID)
            .debug_selector(|| BAND_ID.into())
            .flex()
            .flex_row()
            .flex_none()
            // Centered, where the old band aligned its parts' baselines: the mark is a drawing with no baseline,
            // which would sit below the words.
            .items_center()
            .gap(px(BAND_GAP))
            .px(px(BAND_PAD.0))
            .pb(px(BAND_PAD.1))
            .bg(theme::color(palette.bg_1))
            .children(
                self.status
                    .map(|status| details::badge(status, BAND_STATUS_ID, true, 0, palette)),
            )
            .children(self.age.map(|age| details::age_part(age, BAND_AGE_ID)))
    }
}
