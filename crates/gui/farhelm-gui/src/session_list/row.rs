//! One session's row: its status mark, title and age, its ended status and stale cause when it has them, and where it
//! runs.
//!
//! Ported from the old app's `list/row.rs` (the row body) and the stylesheet's `.session-row` rules at the reference
//! commit. SPEC.md, "Session list": a row's first line shows its status, locality mark, title, agent mark and last
//! activity time; status and locality occupy aligned columns, and times share a right-aligned column before the row
//! menu. A noncompact row shows an ended status in full on a line of its own that wraps, followed on that line by the
//! cause a stale row's status is only last-known ([`status::stale_cause`]); the last line names the host and the
//! working directory as one string, `host:~/path`, starting under the title. The selected row is a solid accent fill
//! with everything on it in the inverse colors ([`Inks`]).
//!
//! The first line is the old identity grid's five tracks, drawn as a flex row with the old gaps: the status slot and
//! the locality slot (the host mark, [`super::row_marks`]) at their fixed width, the title taking what is left, the
//! agent track (the agent mark and the permission shield, two fixed cells, so it is the same width on every row), and
//! the age track ([`age_track`]), at least four of the age's characters wide and right-aligned, so every row's age ends
//! at the same edge and the titles end at the same place whatever the age (up to a thousand days).
//!
//! Everything a host wrote (the title, the host's name, the directory, a stop annotation) is host text: escaped, and
//! each value its own text element, so a direction override in one cannot reorder anything around it.
//!
//! The row's actions menu button sits at its end, outside the open control, so opening the menu never opens the
//! session ([`super::menu`]). A live row's status dot sits inside the open control but is a control of its own: a click
//! on it marks the session read or unread without opening it ([`super::seen`]). A pending Delete shows its spinner and
//! words in place of the title, which stays the row's accessible name ([`super::delete`]); a pending Stop shows its
//! spinner and "Stopping…" before the title, which stays on screen, since the session stays too ([`super::stop`]); a
//! Replace that has waited two seconds says so on a line under the row ([`super::replace`]). The refusal of the row's
//! last request, or the notice its last success left, shows under the row for as long as [`super::outcome`] says, and
//! below that the YOLO question a Replace from the row's menu met ([`super::replace_yolo`]).
//!
//! A session with notifications has a bell before its age, in the age track ([`crate::notices_bell`], which draws it
//! and its list and keeps its press from the open control), so the track widens by the bell on that row only and its
//! title gives way, as the old activity column did with the bell's slot in it.
//! A mute has its own slashed bell ([`super::mute`]). It takes the dot's slot except during a connected wait, when it
//! sits next to the title and leaves the waiting mark visible. An ended row has the mute in its help.
//!
//! **Compact rows.** With the helm-wide compact preference on ([`Steps::compact`]), every row is its first line alone:
//! no ended line and no last line. SPEC.md, "Session list": "The second line is hidden when the helm-wide compact
//! preference is on", and "Compact ended rows replace the live dot with a distinct ended-state icon, preserving complete
//! status details in accessible text and a tooltip". So an ended status draws its mark in the status column
//! ([`status::EndedGlyph`], named and helped by the complete wording, with no mark-read action: SPEC.md's "Ended icons
//! do not have the live dot's mark-read action"), and a stale row draws a clock after its title, named by its cause and
//! helped as the cause is on a noncompact row. The row's hover help is the same summary card as a noncompact row's,
//! which shows the full working directory (the compact menu item says "with the folder shown on hover"), and its
//! description says what the hidden lines said ([`compact_help`]): the complete ended wording, the cause and the full
//! working directory, which the old row kept in hidden text inside its button.
//!
//! **Hover help beside the sidebar.** The row's own hover help is its summary card ([`super::summary`]), shown from the
//! title or any part of the open control no mark covers, and every mark's help asks to sit beside the sidebar too
//! ([`row_marks::HelpPlace`]): SPEC.md, "Session list", puts every session-row hover "just right of the sidebar" with a
//! notch and a ring on its target, the ring in the on-fill ink on the selected row. The innermost control's help wins
//! ([`crate::layers::hover_began`]), so a mark's own help replaces the card while the pointer is on the mark. The bell
//! asks for the same placement in [`crate::notices_bell`], which draws it. Keyboard focus shows the same help as the
//! pointer, in the same place: the card when Tab lands on the open control, and the bell's or the actions button's
//! own help when it lands on one of them, each a shared control that hands its focus stop to its hover help. The other
//! marks are no tab stops (the status dot is a pointer shortcut for the menu's Mark read and Mark unread, the pin grip
//! takes focus only from the press that starts its drag, and the rest are status marks), so their help shows for the
//! pointer only.
//!
//! The row's list item records whether the pointer is over it ([`RowHover`]), for its menu button's reveal
//! ([`super::menu`]).
//!
//! A pinned row draws the muted pin glyph just before its title, and its accessible name starts with "Pinned"
//! ([`super::pins`]). A retired containerized session's title, and its accessible name after any "Pinned", follow
//! "Retired container · " in every density, and its row draws no bell ([`super::retired`]). A movable pin also draws
//! its grip over the open control's leading gutter, and while it is dragged the row is lifted, and the pin a drop would
//! land beside draws the drop line ([`super::pin_drag`]): the item's fill and shadow, and children after the open
//! control so they paint over its hover fill and take the press first.
//!
//! A marked row ([`super::marking`]) draws a 3-pixel marker down its leading edge, in the accent fill, or in the fill's
//! ink on the selected row, whose accent fill it keeps; an unselected marked row is tinted as well (`marked_bg`). The
//! marker is the item's last child, so it paints over the open control's hover fill, as the old `::before` did with
//! `z-index: 1`, and it takes no width from the row. Nothing about the mark reaches assistive technology, as in the
//! old app, whose `data-session-marked` was for its stylesheet and its tests; tests find the marker by its debug
//! selector, `session-marked-<key>`.
//!
//! **An environment session** ([`super::environment`]) draws its provider's mark in the locality slot in place of the
//! host mark (named `Sandbox` or `Container`, [`row_marks::environment_mark`]), and its last line names the host by its
//! environment ("dev sandbox", "dev container on Build machine"). While its environment is suspended the pause-marked
//! Suspended badge stands where a stale row's cause would ("Host suspended"), on the ended line or, compact, after the
//! title; a stopped one keeps its cause ("Environment stopped"). While its environment is being deleted, the host's
//! phase words ("Deleting…" or "Deleting failed") stand in the cause's place on the ended line, helped by why every
//! action is unavailable ([`row_marks::deleting_mark`]); a compact row keeps its stale clock and cause there, as the old
//! compact row did, since it had no being-deleted mark. A pending Suspend, Resume or Stop and suspend, or the
//! provider's change it asked for, shows its spinner and words before the title, as a pending Stop does.
//!
//! NOTE: The locality column keeps its width when a session's host cannot be placed and it draws no mark, so titles
//! start where they did in the old app. The managed checkout's form of the last line (and its compact mark after the
//! title) is not ported yet.

use farhelm_helm_client::Collections;
use farhelm_proto::ui_channel::{HostView, SessionRow};
use gpui::{
    AnyElement, App, Entity, FocusHandle, FontWeight, HoverListenerMode, InteractiveElement,
    IntoElement, ParentElement, SharedString, StatefulInteractiveElement, Styled, Window, div,
    prelude::FluentBuilder as _, px, relative,
};

use super::environment::{self, Environment};
use super::marking::Gesture;
use super::row_marks::{self, HelpPlace, RowMarks};
use super::selection::Listed;
use super::status::{self, Badge};
use super::summary::{self, SummaryCard};
use super::{age, delete, outcome, replace, replace_yolo, stop};
use crate::assets::{self, Icon};
use crate::components::{HoverHelp, ListRow, Role, StatusMark};
use crate::focus;
use crate::host_text::{self, HostSentence, HostText, Line};
use crate::local_time::Zone;
use crate::theme::{self, Palette, metrics};

/// The id prefix of a session row's open control; the rest is the session's key, so the control's focus and hover
/// follow the session when the list reorders.
pub(crate) const ROW_ID_PREFIX: &str = "session-row-";

/// The id prefix of a session's list item, the row around the open control; the rest is the session's key.
pub(crate) const ITEM_ID_PREFIX: &str = "session-item-";

/// The id suffix of a compact row's ended mark, after the row's open control id (`session-row-<key>-ended`).
pub(super) const ENDED_ID_SUFFIX: &str = "-ended";

/// The id suffix of a row's age, after the row's open control id (`session-row-<key>-age`).
pub(super) const AGE_ID_SUFFIX: &str = "-age";

/// The id suffix of a stale row's cause, after the row's open control id (`session-row-<key>-cause`): the words on a
/// noncompact row's ended line, and the clock after a compact row's title, so either is found the same way.
pub(super) const CAUSE_ID_SUFFIX: &str = "-cause";

/// The id suffix of a suspended environment session's badge, after the row's open control id
/// (`session-row-<key>-suspended`), in either density.
pub(super) const SUSPENDED_ID_SUFFIX: &str = "-suspended";

/// The id suffix of the being-deleted mark of a session whose environment is being deleted, after the row's open
/// control id (`session-row-<key>-deleting`).
pub(super) const DELETING_ID_SUFFIX: &str = "-deleting";

/// The id suffix of a row's age track, the column the age is right-aligned in (`session-row-<key>-age-track`).
pub(super) const AGE_TRACK_ID_SUFFIX: &str = "-age-track";

/// The marked row's marker width (`.session-row[data-session-marked="true"]::before`).
const MARKER_WIDTH: gpui::Pixels = px(3.);

/// The advance of every character of the monospace face (the terminal's JetBrains Mono), in ems.
const MONO_ADVANCE_EM: f32 = 0.6;

/// The age track's narrowest width: four characters of the age's monospace face at the row's small text size, the old
/// track's `minmax(4ch, max-content)`. Every age under a thousand days fits inside it, so those rows' titles all end
/// at the same place; a longer age (`1000d`) widens its own track, and the title gives way rather than the age being
/// cut. Ages are right-aligned in the track, so every row's age ends at the same edge either way.
fn age_min_width() -> gpui::Pixels {
    metrics::TEXT_SM * (4. * MONO_ADVANCE_EM)
}

/// The gap that stands for the space closing a retired session's title prefix ([`title_element`]): about a space's
/// advance in the row title's face and size.
const TITLE_PREFIX_GAP: gpui::Pixels = px(4.);

/// The gap between the parts of the ended line: an ended status and a stale cause beside it (`.session-detail-copy`).
const DETAIL_GAP: gpui::Pixels = px(4.);

/// The indent of the lines under the first: the status and locality columns and their gaps (`.session-row-meta`).
fn under_title() -> gpui::Pixels {
    (metrics::ROW_SLOT + metrics::ROW_SLOT_GAP) * 2.
}

/// Whether the pointer is over a row's list item, as the item last reported it: one of its menu button's reveal reasons
/// ([`super::menu`], "The button's reveal"). Element state under the row's key, so it lasts exactly while the row is
/// drawn in consecutive frames and a row drawn again starts unhovered, the way the item's own hover bookkeeping does; a
/// row that scrolls away under a resting pointer cannot leave its button shown. A change redraws the list, which made
/// it.
#[derive(Clone)]
pub(super) struct RowHover(Entity<bool>);

impl RowHover {
    /// `key`'s row's hover state, made while the row is drawn.
    pub(super) fn of(key: &str, window: &mut Window, cx: &mut App) -> RowHover {
        RowHover(window.use_keyed_state(
            SharedString::from(format!("session-row-hover-{key}")),
            cx,
            |_, _| false,
        ))
    }

    /// Record that the pointer is over the row (`hovered`) or not; only a change redraws. A group header records its
    /// hover here too, under its header key, so its grip and menu button reveal the way a row's do.
    pub(super) fn set(&self, hovered: bool, cx: &mut App) {
        self.0.update(cx, |state, cx| {
            if *state != hovered {
                *state = hovered;
                cx.notify();
            }
        });
    }

    /// Whether the pointer is over the row.
    pub(super) fn hovered(&self, cx: &App) -> bool {
        *self.0.read(cx)
    }
}

/// What every row of one frame draws from alike: the steps of the list's two animations (the running pulse's, which
/// only a pulsing status mark uses, and the spinner's, which every pending Delete, Stop and long Replace draws), the
/// wall clock's reading the ages are counted from, and the density.
#[derive(Clone, Copy)]
pub(super) struct Steps {
    pub(super) pulse: u64,
    pub(super) spinner: u8,
    /// The viewer's clock in seconds since the epoch ([`age::seconds`] of [`crate::ticker::wall_now`]), read once per
    /// frame so every row's age is counted from the same instant; `None` when it is unusable, and no row draws an age.
    pub(super) now: Option<i64>,
    /// The viewer's time zone ([`crate::local_time::zone`]), read once per frame with `now`, for the full stamps under
    /// the ages and in the summary card.
    pub(super) zone: Zone,
    /// Whether the rows are compact (see the module docs): the helm-wide preference, or the choice the order menu is
    /// saving while its write is in flight ([`super::order`]). Read once per frame, so no two rows differ.
    pub(super) compact: bool,
}

/// The colors of the text a row draws, which depend on whether the row is the selected one and whether its status is
/// only last-known. On the selected row's solid accent fill the ordinary foregrounds lose their contrast, so it uses the
/// inverse ones (`--on-fill` and the dimmer `--on-accent-dim`), as the old sheet's `.session-row.selected` block did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Inks {
    /// The title: `--fg-0`, or the quieter `--fg-1` on a stale row, where the session's facts are last-known.
    pub(super) title: theme::Token,
    /// The age, the stale cause and the last line: the row's secondary text, `--fg-1`.
    pub(super) secondary: theme::Token,
}

impl Inks {
    /// The inks of a row that is `selected` or not and `stale` or not. A stale row keeps its title readable but
    /// quieter (the old `.session-row.stale .session-title`), selected or not.
    pub(super) fn of(palette: &Palette, selected: bool, stale: bool) -> Inks {
        match (selected, stale) {
            (true, false) => Inks {
                title: palette.on_fill,
                secondary: palette.on_accent_dim,
            },
            (true, true) => Inks {
                title: palette.on_accent_dim,
                secondary: palette.on_accent_dim,
            },
            (false, false) => Inks {
                title: palette.fg_0,
                secondary: palette.fg_1,
            },
            (false, true) => Inks {
                title: palette.fg_1,
                secondary: palette.fg_1,
            },
        }
    }
}

/// Draw `session`'s row, its animations at the list's `steps`, `menu`, its actions menu button, at its end, and `bell`,
/// its notification bell, before its age when it has one ([`crate::notices_bell::bell`]). `drag` is the row's part of a
/// pin drag ([`super::pin_drag::paint`]): its grip, its lift, its drop line and its bounds' recorder. `card` is the
/// row's summary card ([`SummaryCard::of`]), its open control's hover help. `focus` is the row's own focus handle,
/// which the list owns and registers with its list state: the row's item tracks it so it contains whichever of the
/// row's controls holds focus, and never takes focus itself. `hover` is where the item records whether the pointer is
/// over it, for the menu button's reveal. `marked` draws the marked look (see the module docs). `grouped` follows the
/// displayed band, including while the hovered list holds a moved member in its former slot. `on_open` runs when the
/// user activates the row, with the gesture the click's modifiers ask for; the keyboard's Enter and Space carry none,
/// so they always open.
#[allow(
    clippy::too_many_arguments,
    reason = "one row's inputs, all drawn from by one function"
)]
pub(crate) fn row(
    session: Listed,
    collections: &Collections,
    selected: bool,
    marked: bool,
    grouped: bool,
    steps: Steps,
    menu: AnyElement,
    bell: Option<AnyElement>,
    drag: super::pin_drag::RowPaint,
    card: Entity<SummaryCard>,
    focus: &FocusHandle,
    hover: RowHover,
    on_open: impl Fn(Gesture, &mut Window, &mut App) + 'static,
    cx: &App,
) -> AnyElement {
    let palette = theme::palette(cx);
    let row = session.row;
    let compact = steps.compact;
    let id: SharedString = format!("{ROW_ID_PREFIX}{}", session.key).into();
    let title = HostText::from_host(row.info.title.as_str());
    let badge = status::row_badge(row, format!("{id}-status").into());
    let inks = Inks::of(palette, selected, row.stale);
    let marks = RowMarks::of(row, collections, selected, palette);
    let place = HelpPlace::Row { selected };
    // A session in its own environment draws its provider's mark in the locality slot, and a suspended one the
    // Suspended badge where a stale row's cause would be ([`super::environment`]), as one being deleted draws its
    // phase there on a noncompact row.
    let environment = Environment::of(row, collections);
    let suspended = environment.is_some_and(|environment| environment.suspended());
    let deleting = environment.and_then(Environment::deleting);
    let cause = status::stale_cause(row, collections).filter(|_| !suspended);
    let slot_mark = match environment {
        Some(environment) => row_marks::environment_mark(
            &id,
            environment.ui,
            if selected {
                palette.on_fill
            } else {
                palette.fg_1
            },
            place,
        )
        .into_any_element(),
        None => marks
            .host
            .map(|mark| row_marks::host_mark(&id, mark, palette, place).into_any_element())
            .unwrap_or_else(|| div().into_any_element()),
    };
    let pending = environment
        .and_then(|environment| environment::pending(session.key, environment, cx))
        .filter(|_| !stop::in_flight(session.key, cx));
    // The row's accessible name is its title, escaped as the screen shows it, after "Pinned" on a pinned row as the old
    // open button's name read its pin glyph first, and after the retired prefix on a retired session's row, as the old
    // title read. Its hover help is the summary card, beside the sidebar, in either density; the card's words, which are
    // the row's accessible description, are its status word and what a click does, and on a compact row what its hidden
    // lines would have said.
    let retired = super::retired::is(row);
    let mut label = HostSentence::new(id.clone());
    if row.pinned {
        label = label.text("Pinned ");
    }
    if retired {
        label = label.text(super::retired::TITLE_PREFIX);
    }
    let label = label.host(&title).accessible_name();
    let item_label = label.clone();
    let muted = super::mute::label(row, steps.now, steps.zone);
    let words: Line = if compact {
        compact_help(&id, row, cause, muted.as_deref()).into()
    } else {
        match (Badge::word(badge.as_ref()), muted.as_deref()) {
            (Some(word), Some(muted)) => format!("{word} · {muted} — open this session"),
            (None, Some(muted)) => format!("Unclassified · {muted} — open this session"),
            (Some(word), None) => format!("{word} — open this session"),
            (None, None) => "Open this session".to_owned(),
        }
        .into()
    };
    let help = place.place(HoverHelp::card(summary::CARD_WIDTH, card, words));

    let (live, ended) = match badge {
        Some(Badge::Live(live)) => (Some(live), None),
        Some(Badge::Ended { kind, line }) => (None, Some((kind, line))),
        None => (None, None),
    };
    // A compact row's ended status is a mark in the live dot's place; a noncompact row's is its ended line.
    let ended_mark = compact
        .then(|| ended_mark(&id, row, palette, selected, muted.as_deref()))
        .flatten();
    let activity_mark = if super::mute::replaces_dot(row) {
        Some(super::mute::bell(session, selected, steps, cx))
    } else {
        live.map(|live| super::seen::dot(session, live, selected, steps.pulse, cx))
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
                // Mute uses the dot's slot. Only a connected wait keeps
                // its own dot and shows the bell after the title instead.
                .children(activity_mark)
                .children(ended_mark),
        )
        // The locality slot, kept at its width whether or not the session's host can be placed.
        .child(
            div()
                .w(metrics::ROW_SLOT)
                .flex_none()
                .flex()
                .items_center()
                .justify_center()
                .child(slot_mark),
        )
        // A pending Stop goes before the title, as the old `.stop-progress` did, and so does an environment's pending
        // Suspend, Resume or Stop and suspend, in the same look.
        .children(stop::in_flight(session.key, cx).then(|| {
            stop::progress(
                session.key,
                "session-stop-progress",
                stop::PROGRESS_LABEL,
                steps.spinner,
                selected,
                cx,
            )
        }))
        .children(pending.map(|pending| {
            stop::progress(
                session.key,
                "session-environment-progress",
                pending.label(),
                steps.spinner,
                selected,
                cx,
            )
        }))
        // A pinned row's glyph, immediately before the title ([`super::pins`]).
        .children(
            row.pinned
                .then(|| super::pins::glyph(&id, inks.secondary, metrics::ROW_SLOT_GAP, place)),
        )
        // A pending Delete takes the title's place on screen, as the old row's did; the open control and the list item
        // keep the title as their accessible names.
        .child(match delete::in_flight(session.key, cx) {
            Some(guard) => {
                delete::progress(session.key, guard, steps.spinner, selected, cx).into_any_element()
            }
            None => title_element(
                &title,
                retired,
                inks,
                super::mute::after_title(row)
                    .then(|| super::mute::bell(session, selected, steps, cx)),
            ),
        })
        // A compact stale row's clock, after the title, which gives way to it (`.compact-qualifier`), or a compact
        // suspended row's pause mark in its place.
        .children(
            cause
                .filter(|_| compact)
                .map(|cause| stale_mark(&id, cause, palette, selected)),
        )
        .children((compact && suspended).then(|| suspended_mark(&id, false, selected, inks)))
        .child(row_marks::agent_track(&id, &marks, place))
        .child(age_track(
            &id,
            bell,
            age::of(row.info.effective_activity(), steps.now, steps.zone),
            inks,
            place,
        ));

    let indent = if grouped {
        super::groups::MEMBER_INDENT
    } else {
        px(0.)
    };
    let mut control = ListRow::new(id.clone(), label, help, Role::Button)
        .selected(selected)
        .content_indent(indent)
        .line(identity);
    if !compact && (ended.is_some() || cause.is_some() || suspended || deleting.is_some()) {
        // The being-deleted phase takes the cause's place, as the Suspended badge does.
        let cause = cause.filter(|_| deleting.is_none());
        control = control.line(detail_line(
            &id, ended, cause, suspended, deleting, palette, selected, inks,
        ));
    }
    if !compact && let Some(meta) = meta_line(session, collections, inks) {
        control = control.line(meta);
    }
    control = control.on_click(move |event, window, cx| {
        on_open(Gesture::from_modifiers(&event.modifiers()), window, cx)
    });

    // The row itself is the list's item, and it is what carries "selected" for assistive technology: AccessKit
    // reports selection only on item-like roles (a list item, an option), not on a button. The old app marked its open
    // button `aria-current` instead, which GPUI cannot express. The item is named like its button, by the title, so
    // nothing reads out its element id instead.
    let item = div().id(SharedString::from(format!(
        "{ITEM_ID_PREFIX}{}",
        session.key
    )));
    // A press on the item's own space leaves focus where it is (`OnPress::Keep`): the controls inside take their
    // presses first, and the handle is no tab stop.
    focus::track(item, focus, focus::OnPress::Keep)
        .role(Role::ListItem)
        .aria_label(item_label)
        .aria_selected(selected)
        // Hit-tested whatever the last input was, as the old `:hover` was: a key typed while the pointer rests on the
        // row must not hide its menu button.
        .hover_listener_mode(HoverListenerMode::InputModalityIndependent)
        .on_hover(move |hovered, _, cx| hover.set(*hovered, cx))
        .relative()
        .w_full()
        .border_b_1()
        .border_color(theme::color(palette.border_row))
        .when(selected, |row| row.bg(theme::color(palette.accent_fill)))
        .when(marked && !selected, |row| {
            row.bg(theme::color(palette.marked_bg))
        })
        // After the selected and marked fills, which the lift replaces while it lasts. Tests find a lifted row by its
        // debug selector, `session-pin-lifted-<key>`.
        .when(drag.lifted, |row| {
            let selector = format!("session-pin-lifted-{}", session.key);
            row.bg(super::pin_drag::lifted_fill(palette, selected))
                .shadow(super::pin_drag::lifted_shadow(palette))
                .debug_selector(move || selector.clone())
        })
        .child(
            div()
                .flex()
                .flex_row()
                .gap(px(6.))
                .child(div().flex_1().min_w_0().child(control))
                .child(menu),
        )
        .children(
            replace::progress_line(session.key, row, steps.spinner, selected, cx)
                .map(|line| inset_inline(line, indent)),
        )
        .children(
            outcome::row_line(session.key, selected, cx).map(|line| inset_inline(line, indent)),
        )
        .children(
            replace_yolo::row_question(session.key, cx)
                .map(|question| inset_inline(question, indent)),
        )
        .children(grouped.then(|| {
            // The row's bottom border is outside the absolute positioning area. Carry the guide through that pixel.
            super::groups::member_guide(palette)
                .bottom(-px(1.))
                .debug_selector(|| format!("session-group-guide-{}", session.key))
        }))
        .children(drag.recorder)
        .children(drag.grip)
        .children(drag.drop_line)
        .children(marked.then(|| marker(session.key, selected, palette)))
        .into_any_element()
}

/// Inline notices and questions keep their own padding but begin past the same group guide as the open control.
fn inset_inline(content: impl IntoElement, indent: gpui::Pixels) -> AnyElement {
    if indent == px(0.) {
        content.into_any_element()
    } else {
        div().pl(indent).child(content).into_any_element()
    }
}

/// The row's title as the first line draws it, taking the width the line leaves and truncated at its end, in the
/// row's title ink. A retired session's title follows [`super::retired::TITLE_PREFIX`] in the same face and ink, in
/// every density (the old app prefixed the title itself): the prefix keeps its width and the host's title, a value of
/// its own, gives way, so the retirement stays readable however narrow the sidebar is. The two are separate text
/// elements, since a host value is always one of its own, so the prefix's closing space is drawn as a gap between them
/// rather than left to each shaper's handling of a run's trailing space.
/// A waiting row's bell sits against the title while the group takes the
/// remaining width. Only the title text gives way when space runs short.
fn title_element(
    title: &HostText,
    retired: bool,
    inks: Inks,
    bell: Option<AnyElement>,
) -> AnyElement {
    let styled = |element: gpui::Div| {
        element
            .text_size(metrics::TEXT_ROW_TITLE)
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(theme::color(inks.title))
    };
    let host = host_text::element(title).min_w_0().truncate();
    if !retired && bell.is_none() {
        return styled(host.flex_1()).into_any_element();
    }
    styled(
        div()
            .flex_1()
            .min_w_0()
            .flex()
            .flex_row()
            .when(bell.is_some(), |title| title.items_center())
            .gap(TITLE_PREFIX_GAP),
    )
    .children(retired.then(|| {
        div()
            .flex_none()
            .whitespace_nowrap()
            .child(super::retired::TITLE_PREFIX.trim_end())
    }))
    .child(if bell.is_some() {
        host.flex_initial()
    } else {
        host.flex_1()
    })
    .children(bell)
    .into_any_element()
}

/// A marked row's marker: [`MARKER_WIDTH`] down the row's whole leading edge, over everything else in it, in the
/// accent fill, or in the fill's ink on the selected row's accent fill. No listeners, so it takes no presses.
fn marker(key: &str, selected: bool, palette: &Palette) -> impl IntoElement {
    let selector = format!("session-marked-{key}");
    div()
        .absolute()
        .top_0()
        .bottom_0()
        .left_0()
        .w(MARKER_WIDTH)
        .bg(theme::color(if selected {
            palette.on_fill
        } else {
            palette.accent_fill
        }))
        .debug_selector(move || selector.clone())
}

/// The first line's last track: the session's age in the row's secondary ink, right-aligned in a track at least
/// [`age_min_width`] wide that never shrinks, with the full timestamp as its hover help. The track stays (empty) for a
/// session with no age, so that row's title still ends where the others' do. `bell`, the row's notification bell when
/// it has one, goes first in the track, just before the age, where the old activity column reserved the bell's slot;
/// the two are centred on one line. The age's help shows where `place` says, beside the sidebar.
///
/// The age is a status mark, not a control: nothing to press, so no tab stop, and its press reaches the open control
/// around it, which opens the row as a press anywhere else on it does. Its accessible name is the timestamp itself,
/// since `2m` read aloud says little; its hover help is the same words, so it is not read twice. Like the old
/// `.status-time`, it is drawn in the monospace face, so ages of the same length are the same width.
fn age_track(
    row_id: &SharedString,
    bell: Option<AnyElement>,
    age: Option<age::Age>,
    inks: Inks,
    place: HelpPlace,
) -> impl IntoElement {
    div()
        .id(SharedString::from(format!("{row_id}{AGE_TRACK_ID_SUFFIX}")))
        .flex_none()
        .min_w(age_min_width())
        .flex()
        .flex_row()
        .items_center()
        .justify_end()
        .children(bell)
        .children(age.map(|age| {
            StatusMark::new(
                format!("{row_id}{AGE_ID_SUFFIX}"),
                age.full.clone(),
                place.place(HoverHelp::tooltip(age.full)),
                Role::Label,
                age.short,
            )
            .ink(inks.secondary)
            .monospace()
            .described(false)
        }))
}

// ---- Compact rows ---------------------------------------------------------------------------------------------------

/// A compact row's ended mark in the status column (`.compact-ended-status`), or `None` for a status that is not ended:
/// the shape of how the session ended ([`status::EndedGlyph`]) in its ink, named by the complete ended wording the
/// noncompact row shows on its own line (`Exited (code unknown) — stopped by user`) and helped by the same words. A
/// status mark, not a control: no mark-read action, no tab stop, and a press on it opens the row as a press anywhere
/// else on the row does. Its help shows beside the sidebar, as every row hover's.
fn ended_mark(
    row_id: &SharedString,
    row: &SessionRow,
    palette: &Palette,
    selected: bool,
    muted: Option<&str>,
) -> Option<impl IntoElement + use<>> {
    let glyph = status::EndedGlyph::of(row)?;
    let words = status::ended_words(row, true)?;
    let id = SharedString::from(format!("{row_id}{ENDED_ID_SUFFIX}"));
    // The words quote the host after the app's own (a stop annotation, a launch failure's detail), so help naming them
    // is a sentence, which escapes them as the screen does.
    let sentence = |id| {
        let mut line = words.sentence(id);
        if let Some(muted) = muted {
            line = line.text(" · ").text(muted);
        }
        line
    };
    let help = match (&words.host, muted) {
        (None, None) => HoverHelp::tooltip(words.words.clone()),
        _ => HoverHelp::sentence(sentence(SharedString::from(format!("{id}-help")))),
    };
    let help = HelpPlace::Row { selected }.place(help);
    let name = sentence(id.clone()).accessible_name();
    let ink = glyph.color(palette, selected);
    Some(
        StatusMark::new(id.clone(), name.clone(), help, Role::Image, name)
            .described(false)
            .content(row_marks::cell(id, glyph.icon(), ink)),
    )
}

/// A compact stale row's clock after its title (`.compact-qualifier`), named by the row's cause and helped as the cause
/// is on a noncompact row (`Waiting for supervisor: this is the session's last-known state`). Its id is the cause's on a
/// noncompact row, so either is found the same way. Its help shows beside the sidebar, as every row hover's.
fn stale_mark(
    row_id: &SharedString,
    cause: &'static str,
    palette: &Palette,
    selected: bool,
) -> impl IntoElement + use<> {
    let id = SharedString::from(format!("{row_id}{CAUSE_ID_SUFFIX}"));
    div().flex_none().child(
        StatusMark::new(
            id.clone(),
            cause,
            HelpPlace::Row { selected }.place(HoverHelp::tooltip(status::cause_help(cause))),
            Role::Status,
            cause,
        )
        .content(row_marks::cell(
            id,
            Icon::Stale,
            status::stale_mark_color(palette, selected),
        )),
    )
}

/// A compact row's words for its summary card, which are its accessible description and what the tooltip reports: what
/// its hidden lines would have said, then what a click does. The card itself shows the folder; these words keep the
/// complete ended wording and the cause for assistive technology, as SPEC.md asks of compact rows. Its parts, each
/// only when the row has it, joined by ` · `: the status (the complete ended wording, or the live word), a stale row's
/// cause, and the full working directory (unabbreviated, as the old row's hidden copy gave it); then ` — open this
/// session`. For example `Exited (code 17) · Waiting for supervisor · /home/user/src/a — open this session`. A
/// sentence, since the wording and the directory are host text.
fn compact_help(
    row_id: &SharedString,
    row: &SessionRow,
    cause: Option<&'static str>,
    muted: Option<&str>,
) -> HostSentence {
    let mut help = HostSentence::new(SharedString::from(format!("{row_id}-help")));
    let mut first = true;
    let mut separate = |help: HostSentence| {
        if std::mem::take(&mut first) {
            help
        } else {
            help.text(" · ")
        }
    };
    if let Some(ended) = status::ended_words(row, true) {
        help = separate(help).text(ended.words);
        if let Some(host) = &ended.host {
            help = help.host(host);
        }
    } else if let Some(live) = status::live(row) {
        help = separate(help).text(live.word());
    } else if muted.is_some() {
        help = separate(help).text("Unclassified");
    }
    if let Some(muted) = muted {
        help = separate(help).text(muted);
    }
    if let Some(cause) = cause {
        help = separate(help).text(cause);
    }
    if !row.info.cwd.is_empty() {
        help = separate(help).mono(&HostText::from_host(row.info.cwd.as_str()));
    }
    if first {
        help.text("Open this session")
    } else {
        help.text(" — open this session")
    }
}

/// The row's Suspended mark ([`row_marks::suspended_mark`]) under `<row id>-suspended`, worded on the ended line and
/// the glyph alone after a compact row's title, in the row's secondary ink, its help beside the sidebar.
fn suspended_mark(
    row_id: &SharedString,
    worded: bool,
    selected: bool,
    inks: Inks,
) -> impl IntoElement {
    row_marks::suspended_mark(
        SharedString::from(format!("{row_id}{SUSPENDED_ID_SUFFIX}")),
        worded,
        inks.secondary,
        HelpPlace::Row { selected },
    )
}

/// The line under the title that holds what the first line has no room for: an ended status in full, wrapping rather
/// than truncating (`.session-row-detail`), then a stale row's cause, on the same wrapping line when it fits, or in
/// the cause's place the Suspended badge (`suspended`) or the environment's being-deleted phase (`deleting`); the
/// caller passes at most one of the three, as the old row's `if … else if` chain drew one. Drawn only for a row that
/// has an ended status or one of them; a live row on a connected host keeps two lines.
///
/// The cause is a status mark with its hover help (`<cause>: this is the session's last-known state`), the old
/// `.stale-badge` capsule with the capsule's padding taken off inside this line, so it starts at the title's edge like
/// the ended words do; it never wraps inside itself.
#[allow(
    clippy::too_many_arguments,
    reason = "one line's parts and the row's inks, all drawn from by one function"
)]
fn detail_line(
    row_id: &SharedString,
    ended: Option<(status::Ended, host_text::Line)>,
    cause: Option<&'static str>,
    suspended: bool,
    deleting: Option<environment::Deleting>,
    palette: &Palette,
    selected: bool,
    inks: Inks,
) -> impl IntoElement {
    div()
        .pl(under_title())
        .flex()
        .flex_row()
        .flex_wrap()
        .items_baseline()
        .gap(DETAIL_GAP)
        .min_w_0()
        // The ended words set their own 16-pixel lines; the cause takes the same, so it adds no more height than they
        // do and sits on their baseline.
        .line_height(px(16.))
        .children(
            ended.map(|(kind, line)| {
                status::ended_line(kind, line, palette, selected).flex_shrink_1()
            }),
        )
        .children(cause.map(|cause| {
            div().flex_none().child(
                StatusMark::new(
                    format!("{row_id}{CAUSE_ID_SUFFIX}"),
                    cause,
                    HelpPlace::Row { selected }
                        .place(HoverHelp::tooltip(status::cause_help(cause))),
                    Role::Status,
                    cause,
                )
                .ink(inks.secondary),
            )
        }))
        .children(suspended.then(|| suspended_mark(row_id, true, selected, inks)))
        .children(deleting.map(|deleting| {
            row_marks::deleting_mark(
                SharedString::from(format!("{row_id}{DELETING_ID_SUFFIX}")),
                deleting,
                inks.secondary,
                HelpPlace::Row { selected },
            )
        }))
}

/// The `host:~/path` line: the host's short name (or the helm's name for a host the registry no longer lists), a
/// colon, and the working directory with the user's home folded to `~`, cut from the left when it does not fit so the
/// end of the path stays visible; the directory is in the monospace face, like the age. With no host name the line is
/// the directory alone, and with no directory the host alone, never a dangling separator; with neither there is no line
/// at all (`None`), since an empty line would only add height. SPEC.md, "Session list": the row shows only the
/// directory "rather than inventing a local identity or a dangling separator".
fn meta_line(session: Listed, collections: &Collections, inks: Inks) -> Option<impl IntoElement> {
    let row = session.row;
    let (host, cwd) = meta_parts(row, collections.get::<HostView>(&HostView::key(row.host)));
    // An environment session's host goes by its environment's name ("dev sandbox"), not the registry's.
    let host = Environment::of(row, collections)
        .map(Environment::label)
        .or(host);
    if host.is_none() && cwd.is_none() {
        return None;
    }
    let host = host.map(HostText::from_host);
    let cwd = cwd.map(HostText::from_host);
    let separator = host.is_some() && cwd.is_some();
    Some(
        div()
            .flex()
            .flex_row()
            .min_w_0()
            .pl(under_title())
            .text_size(metrics::TEXT_SM)
            .line_height(px(16.))
            .text_color(theme::color(inks.secondary))
            // The host keeps to 40% of the line, so a long destination cannot hide the directory (`.session-host`).
            .children(host.map(|host| {
                host_text::element(&host)
                    .flex_none()
                    .max_w(relative(0.4))
                    .truncate()
            }))
            .when(separator, |line| line.child(div().flex_none().child(":")))
            .children(cwd.map(|cwd| {
                // The directory alone is a technical value, drawn in the monospace face as the old `.session-cwd`
                // was (the host's name and the colon keep the interface face).
                host_text::element(&cwd)
                    .font_family(assets::TERMINAL_FONT)
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_ellipsis_start()
            })),
    )
}

/// The two parts of a row's last line, as raw host text for the caller to escape: the host's name ([`host_label`])
/// and the working directory with the home folded ([`abbreviate_home`]), each `None` when it is empty.
pub(super) fn meta_parts(
    row: &SessionRow,
    host: Option<&HostView>,
) -> (Option<String>, Option<String>) {
    let host = Some(host_label(row, host)).filter(|name| !name.is_empty());
    let cwd = Some(abbreviate_home(&row.info.cwd)).filter(|cwd| !cwd.is_empty());
    (host, cwd)
}

/// The host part of a row's `host:~/path` line, as the old app named it: the host's short name when the registry lists
/// the host ([`short_name`]), else the name the helm sent with the row. Empty only when both are, and the line then
/// shows the directory alone.
pub(crate) fn host_label(row: &SessionRow, host: Option<&HostView>) -> String {
    host.map(short_name)
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| row.host_name.clone())
}

/// A registered host's short name for a row's host, the host rail's caption and the filter chip, as raw host text the
/// caller escapes: [`crate::hosts::identity::short_name`], the rule the attention list and the host progress card use
/// too, so no two surfaces name one host differently. That is its alias, `Local`, or the name derived from an SSH
/// destination, else the registry's own name, which is what a sandbox or container host without an alias reads (that
/// function's docs say why its destination is not used).
pub(crate) fn short_name(host: &HostView) -> String {
    crate::hosts::identity::short_name(host).raw().to_owned()
}

/// `cwd` with a personal home folded to `~` (`/home/alice/src` becomes `~/src`), or unchanged. A path operation on the
/// host's own bytes; the caller escapes the result.
///
/// Ported from the old app's `abbreviate_home`, with its three exclusions: a `.` or `..` segment anywhere (the path
/// would not resolve under that home, so folding it would claim something the path contradicts), macOS's
/// `/Users/Shared` (not anyone's home), and a bare account with nothing after it (folding would erase the one thing the
/// path says). The row only shortens; the full path is what its summary card shows ([`super::summary`]).
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
