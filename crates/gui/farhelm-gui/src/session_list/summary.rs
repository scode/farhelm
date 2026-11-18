//! A session row's hover summary card: the facts the row abbreviates or leaves out, on one card beside the sidebar.
//!
//! SPEC.md, "Session list": "The summary card names the title, status, agent, host, checkout when managed, directory,
//! start and last-activity times, and what a click does", and "the same card appears from the title or any unmarked
//! part of the open button, in either density and on keyboard focus". Ported from the old app's
//! `list/summary_card.rs` and the stylesheet's `.session-summary*` rules at the reference commit, row for row and label
//! for label. Here's the TLDR of what it shows:
//!
//! - **A header**: the title, bold and cut at eight lines, with the status beside it (a live status's mark and word,
//!   or an ended status's word alone, `Exited` without its code, since the card is no place for a long failure message
//!   to crowd out the title), and under them the agent line: the agent, its stored permission and the choices it set
//!   explicitly (`Codex · Guarded · Model: gpt-6 · Effort: High`), or a command's program, then `, YOLO observed` when
//!   the shield has status-line evidence ([`agent_line`]). A stale row's status is grey and still, as on its row.
//! - **The facts**, each under its label and only when the session has it ([`Fact`]): Host (the host's mark in its own
//!   color, never the selected row's inverse ink, and the helm's full name for it), Checkout (the repository and the
//!   checkout's folder), Directory (the full path, unabbreviated: SPEC.md says an abbreviation is never the only place
//!   a value is recorded, and this is where the row's `~` form is recorded in full), Started and Last active (the
//!   relative age and the stamp), each stamp in the viewer's local time ([`crate::local_time`]).
//! - **A footer**: "Click to open" and "⋯ For actions".
//!
//! What the card shows is decided by [`Summary::of`], a pure function of the session's row, the registry and the
//! clock, so tests read it without a window; [`SummaryCard`] only lays it out.
//!
//! The card is hover help of the card form ([`HoverHelp::card`](crate::components::HoverHelp::card)), so the layer
//! manager draws the view, only while the card shows, and the row never draws it inline. One view per row, kept as the
//! row's element state ([`SummaryCard::of`]): the help compares the view's entity, so a new one every frame would make
//! the row's help look changed on every frame. The view keeps only the session's key and the frame's animation steps; it
//! reads the session from the store as it draws, so it never shows facts older than the row beside it.
//!
//! The card also shows when keyboard focus reaches the row's open control, as SPEC.md and the old app have it. The
//! open control is a [`ListRow`](crate::components::ListRow), which hands its own focus stop to its hover help, so
//! the layer manager shows the card for a keyboard landing as it does for every focusable control's help: after the
//! same hover delay as the pointer's (or at once in the grace another tooltip left), beside the sidebar with its notch
//! and ring, gone when focus leaves the control, and put away by Escape or a press until focus next lands. Nothing
//! here asks for it, so a row that stopped drawing its open control as a `ListRow` would lose it silently;
//! `summary_tests.rs` pins it. A mark that holds focus inside the row (the bell, the actions button) shows its own help
//! instead, since only the open control's own stop counts.
//!
//! The card is for the eye only. A screen reader hears the row's own name and description (its title, then its status
//! and what a click does, and on a compact row what its hidden lines said), as the old card's hidden source stayed out
//! of the accessibility tree so the title, status and directory were not read twice.

use farhelm_helm_client::Collections;
use farhelm_proto::SessionLaunch;
use farhelm_proto::ui_channel::SessionRow;
use gpui::{
    AnyElement, App, Context, Entity, FontWeight, InteractiveElement as _, IntoElement,
    ParentElement, Pixels, Render, SharedString, Styled, Window, div, prelude::FluentBuilder as _,
    px, relative,
};

use super::age;
use super::menu;
use super::row::Steps;
use super::row_marks::{self, HostMark, Locality};
use super::status::{self, Ended, Live};
use crate::assets;
use crate::host_text::{self, HostSentence, HostText};
use crate::hosts::identity;
use crate::launcher::draft::harness_label;
use crate::launcher::recents::explicit_before_permissions;
use crate::local_time::{self, Zone};
use crate::permission::observed_yolo_mode;
use crate::permission::words;
use crate::session;
use crate::theme::{self, Palette};

/// The card's width, the old `.farhelm-tooltip[data-kind="card"]`'s `min(310px, 100vw - 8px)` (the layer manager keeps
/// it 8 pixels inside a narrower window).
pub(crate) const CARD_WIDTH: Pixels = px(310.);

/// The id prefix of a session's card; the rest is the session's key. Tests find the card's facts under it.
pub(crate) const SUMMARY_ID_PREFIX: &str = "session-summary-";

/// The footer's left half: what a click on the row does.
pub(crate) const CLICK_HINT: &str = "Click to open";

/// The footer's right half: where the row's other actions are.
pub(crate) const ACTIONS_HINT: &str = "⋯ For actions";

/// The permission value of an agent launch that stores none, the agent running under its own setting (the old
/// `menu_header_summary_parts`' fallback). The launcher's footer says "Default" for the same thing, which reads as a
/// choice Farhelm made; here it is the agent's.
const AGENT_OWN_SETTING: &str = "Agent's own setting";

/// The label column's width: `.session-summary-row`'s `grid-template-columns: 6rem …` at the old page's 16-pixel rem.
const LABEL_WIDTH: Pixels = px(96.);

/// The agent line's and footer's text size (`.session-summary-agent`, `.session-summary-footer`).
const SMALL_TEXT: Pixels = px(11.);

/// The host mark's size in the card (`.host-kind-icon`).
const HOST_MARK_SIZE: Pixels = px(12.);

// ---- What the card shows --------------------------------------------------------------------------------------------

/// A session's status as the card names it: the short word, without an exit code or a host's details (the old
/// `StatusBadge::label`), and for a live status its mark.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CardStatus {
    Live(Live),
    Ended(Ended),
}

impl CardStatus {
    /// The status word: `Running`, `Idle — new output`, `Exited`.
    pub(crate) fn word(self) -> &'static str {
        match self {
            CardStatus::Live(live) => live.word(),
            CardStatus::Ended(ended) => ended.word(),
        }
    }

    /// The status's ink on the neutral card: its own color, or the quiet `--fg-2` on a `stale` row, whose status is
    /// only last-known (`.session-summary.stale .session-summary-status`).
    fn ink(self, palette: &Palette, stale: bool) -> theme::Token {
        match self {
            CardStatus::Live(live) => live.mark_color(palette, false, stale),
            CardStatus::Ended(_) if stale => palette.fg_2,
            CardStatus::Ended(ended) => ended.color(palette, false),
        }
    }
}

/// One of the card's labelled facts, in the order the card lists them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Fact {
    /// Where the session runs: the host's mark when the registry can place the host, and the helm's full name for it
    /// when the helm sent one (`Local (this machine)` for the local host under the registry's own name). At least one
    /// of the two is there, or the card has no Host fact.
    Host {
        mark: Option<HostMark>,
        name: Option<HostText>,
    },
    /// The managed checkout: its repository (`owner/name`) and its folder's name.
    Checkout { repo: HostText, folder: HostText },
    /// The working directory, in full.
    Directory(HostText),
    /// When the session was created, as a stamp in the viewer's zone.
    Started(String),
    /// When the agent was last active: the relative age and the stamp in the viewer's zone, `2m · 2023-11-14 22:13:20`.
    LastActive(String),
}

impl Fact {
    /// The fact's label, the old card's word for word.
    pub(crate) const fn label(&self) -> &'static str {
        match self {
            Fact::Host { .. } => "Host",
            Fact::Checkout { .. } => "Checkout",
            Fact::Directory(_) => "Directory",
            Fact::Started(_) => "Started",
            Fact::LastActive(_) => "Last active",
        }
    }

    /// The fact's value as one sentence under `id`, host values escaped: what the card draws beside the label (after
    /// the host's mark, for Host), and, through its accessible name, what a test reads.
    pub(crate) fn value(&self, id: SharedString) -> HostSentence {
        let sentence = HostSentence::new(id);
        match self {
            Fact::Host { name, .. } => match name {
                Some(name) => sentence.host(name),
                None => sentence,
            },
            Fact::Checkout { repo, folder } => sentence.host(repo).text(" · ").mono(folder),
            Fact::Directory(path) => sentence.mono(path),
            Fact::Started(stamp) | Fact::LastActive(stamp) => sentence.text(stamp.clone()),
        }
    }
}

/// Everything a session's card shows (see the module docs), decided from its row, the registry and the clock.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Summary {
    pub(crate) title: HostText,
    /// `None` for a status nobody classified, which the card leaves out as the row does.
    pub(crate) status: Option<CardStatus>,
    /// Whether the session's status is only last-known (its host is not connected).
    pub(crate) stale: bool,
    /// A row's mute wording in this viewer's clock, beside its real status.
    muted: Option<String>,
    /// The agent line, a sentence under `id`: [`agent_line`].
    pub(crate) agent: HostSentence,
    pub(crate) facts: Vec<Fact>,
}

impl Summary {
    /// `row`'s card, its sentences under ids made from `key`, with the viewer's clock at `now` (seconds since the epoch,
    /// [`age::seconds`]; `None` when it cannot be used, and the card has no Last active, as the row has no age) and its
    /// stamps written in `zone`, the viewer's.
    pub(crate) fn of(
        key: &str,
        row: &SessionRow,
        collections: &Collections,
        now: Option<i64>,
        zone: Zone,
    ) -> Summary {
        let status = status::live(row)
            .map(CardStatus::Live)
            .or_else(|| status::ended_words(row, false).map(|ended| CardStatus::Ended(ended.kind)));
        let mut facts = Vec::new();
        let mark = HostMark::of(row, collections, None);
        let local = mark.is_some_and(|mark| mark.locality == Locality::Local);
        let name = (!row.host_name.is_empty()).then(|| identity::named(&row.host_name, local));
        if mark.is_some() || name.is_some() {
            facts.push(Fact::Host { mark, name });
        }
        if let Some(copy) = &row.info.working_copy {
            // The folder's name is cut from the path as the host spelled it, before it is escaped: an escape's
            // notation is how the path is shown, not where it splits. A trailing slash names no folder of its own.
            let path = copy.canonical_path.as_str();
            let folder = path
                .rsplit('/')
                .find(|segment| !segment.is_empty())
                .unwrap_or(path);
            facts.push(Fact::Checkout {
                repo: HostText::from_host(format!("{}/{}", copy.repo.owner, copy.repo.name)),
                folder: HostText::from_host(folder),
            });
        }
        if !row.info.cwd.is_empty() {
            facts.push(Fact::Directory(HostText::from_host(row.info.cwd.as_str())));
        }
        if row.info.created_at > 0 {
            facts.push(Fact::Started(local_time::stamp(row.info.created_at, zone)));
        }
        let activity = row.info.effective_activity();
        if let Some(age) = age::of(activity, now, zone) {
            facts.push(Fact::LastActive(format!(
                "{} · {}",
                age.short,
                local_time::stamp(activity, zone)
            )));
        }
        Summary {
            title: HostText::from_host(row.info.title.as_str()),
            status,
            stale: row.stale,
            muted: super::mute::label(row, now, zone),
            agent: agent_line(
                SharedString::from(format!("{SUMMARY_ID_PREFIX}{key}-agent")),
                row,
            ),
            facts,
        }
    }
}

/// The card's agent line, a sentence under `id`: the old `summary_parts`. An agent launch names its agent type as the
/// launcher does (`Claude`, not the row's `Claude Code`), its stored permission (or the agent's own setting) and the
/// choices it set explicitly before the permission (its model, effort and a retired container, as the launcher's
/// recent setups name them, [`explicit_before_permissions`]), joined by ` · `. Any
/// other launch names the program its recorded command line runs, in the monospace face. Then `, YOLO observed` when
/// the agent's status line was seen in a mode without checks, even for a launch that was YOLO already: seeing a mode
/// and choosing it are different facts. The actions menu's header describes only the stored launch; this line is
/// where the observation shows besides the shield.
pub(crate) fn agent_line(id: SharedString, row: &SessionRow) -> HostSentence {
    let launch = &row.info.launch;
    let mut line = HostSentence::new(id);
    match launch {
        SessionLaunch::Agent { selection, .. } => {
            let permission = selection
                .harness
                .stored_permission(selection.permissions)
                .map_or(AGENT_OWN_SETTING, |permission| {
                    words::permission_display(Some(permission))
                });
            line = line
                .text(harness_label(selection.harness))
                .text(" · ")
                .text(permission);
            // The launcher's one rule for which choices are explicit and how each is worded (`Model: gpt-6`), each a
            // whole value escaped as the old card's peer runs were: the model is the host's text.
            for choice in explicit_before_permissions(selection) {
                line = line.text(" · ").host(&HostText::from_host(choice));
            }
        }
        SessionLaunch::Command(_) | SessionLaunch::Legacy { .. } => {
            line = line.mono(&HostText::from_host(menu::program_name(
                &row.info.invocation,
            )));
        }
    }
    if observed_yolo_mode(launch, row.info.screen_yolo).is_some() {
        line = line.text(", YOLO observed");
    }
    line
}

// ---- The view -------------------------------------------------------------------------------------------------------

/// A session's summary card as a view: what the row hands its hover help
/// ([`HoverHelp::card`](crate::components::HoverHelp::card)), drawn by the layer manager while the card shows.
pub(crate) struct SummaryCard {
    key: String,
    /// The frame's animation steps and clock, as the row was last drawn with them, so the card's running mark pulses
    /// with the row's and its age is counted from the same instant. The list holds the pulse and the clock that redraw
    /// the window; the card holds neither, since it draws only while its row does.
    steps: Steps,
}

impl SummaryCard {
    /// The card of `key`'s row, drawn with this frame's `steps`: made on the row's first frame and kept while the row is
    /// drawn in consecutive frames (element state under the row's key, as the row's hover state is). Called from the
    /// list's item callback.
    pub(super) fn of(
        key: &str,
        steps: Steps,
        window: &mut Window,
        cx: &mut App,
    ) -> Entity<SummaryCard> {
        let card = window.use_keyed_state(
            SharedString::from(format!("{SUMMARY_ID_PREFIX}{key}-view")),
            cx,
            |_, _| SummaryCard {
                key: key.to_owned(),
                steps,
            },
        );
        // No notify: the card draws only while the tooltip shows, in the frame being drawn, and a step that changes
        // redraws the window anyway (the list's pulse and clock).
        card.update(cx, |card, _| card.steps = steps);
        card
    }
}

impl Render for SummaryCard {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(store) = session::current_store(cx) else {
            return div().into_any_element();
        };
        let collections = store.read(cx).collections();
        let Some(row) = collections.get::<SessionRow>(&self.key) else {
            return div().into_any_element();
        };
        let summary = Summary::of(&self.key, row, collections, self.steps.now, self.steps.zone);
        draw(&self.key, &summary, theme::palette(cx), self.steps.pulse)
    }
}

/// The card's drawing: the old `.session-summary` header, body and footer, the running mark at pulse step `pulse`.
fn draw(key: &str, summary: &Summary, palette: &Palette, pulse: u64) -> AnyElement {
    let id = |part: &str| SharedString::from(format!("{SUMMARY_ID_PREFIX}{key}{part}"));
    let pulse = if summary.muted.is_some() { 0 } else { pulse };
    let status = summary.status.map(|status| {
        let ink = status.ink(palette, summary.stale);
        div()
            .id(id("-status"))
            .flex()
            .flex_row()
            .flex_initial()
            .items_center()
            .gap(px(5.))
            .max_w(relative(0.45))
            .text_color(theme::color(ink))
            .when_some(
                match status {
                    CardStatus::Live(live) => Some(live),
                    CardStatus::Ended(_) => None,
                },
                |status, live| status.child(live.mark(palette, false, summary.stale, pulse)),
            )
            .child(div().min_w_0().child(status.word()))
    });
    let header = div()
        .px(px(12.))
        .pt(px(9.))
        .pb(px(8.))
        .border_b_1()
        .border_color(theme::color(palette.border_dim))
        .child(
            div()
                .flex()
                .flex_row()
                .items_start()
                .gap(px(9.))
                .min_w_0()
                .child(
                    div()
                        .id(id("-title"))
                        .flex_1()
                        .min_w_0()
                        .line_clamp(8)
                        .font_weight(FontWeight::BOLD)
                        .text_color(theme::color(palette.fg_0))
                        .child(host_text::element(&summary.title)),
                )
                .children(status),
        )
        .child(
            div()
                .mt(px(3.))
                .text_size(SMALL_TEXT)
                .text_color(theme::color(palette.fg_1))
                .child(summary.agent.clone()),
        )
        .children(summary.muted.as_ref().map(|muted| {
            div()
                .id(id("-mute"))
                .mt(px(3.))
                .text_size(SMALL_TEXT)
                .text_color(theme::color(palette.fg_1))
                .child(muted.clone())
        }));
    let body = div()
        .flex()
        .flex_col()
        .gap(px(6.))
        .px(px(12.))
        .py(px(9.))
        .children(
            summary
                .facts
                .iter()
                .enumerate()
                .map(|(index, fact)| fact_row(fact, id(&format!("-fact-{index}")), palette)),
        );
    let footer = div()
        .id(id("-footer"))
        .flex()
        .flex_row()
        .justify_between()
        .gap(px(12.))
        .px(px(12.))
        .pt(px(7.))
        .pb(px(8.))
        .border_t_1()
        .border_color(theme::color(palette.border_dim))
        .text_size(SMALL_TEXT)
        .text_color(theme::color(palette.fg_2))
        .child(div().child(CLICK_HINT))
        .child(div().child(ACTIONS_HINT));
    div()
        .id(id(""))
        .min_w_0()
        .text_color(theme::color(palette.fg_0))
        .child(header)
        .child(body)
        .child(footer)
        .into_any_element()
}

/// One `.session-summary-row`: the label in its 96-pixel column, the value beside it wrapping in what is left. The
/// directory is in the dimmer `--fg-dim` (`.session-summary-path`), the times in the monospace face (`font-mono`), and
/// the host's value leads with its mark (`.session-summary-host`).
fn fact_row(fact: &Fact, id: SharedString, palette: &Palette) -> impl IntoElement {
    let value = fact.value(SharedString::from(format!("{id}-value")));
    let cell = div().flex_1().min_w_0();
    let cell = match fact {
        Fact::Host { mark, .. } => cell
            .flex()
            .flex_row()
            .items_center()
            .gap(px(5.))
            .children(mark.map(|mark| {
                let picture = id.clone();
                identity::host_mark(mark.icon, mark.color, HOST_MARK_SIZE, palette)
                    .flex_none()
                    .debug_selector(move || {
                        row_marks::picture_selector(&picture, mark.icon.as_str())
                    })
            }))
            .child(value),
        Fact::Directory(_) => cell.text_color(theme::color(palette.fg_dim)).child(value),
        Fact::Started(_) | Fact::LastActive(_) => {
            cell.font_family(assets::TERMINAL_FONT).child(value)
        }
        Fact::Checkout { .. } => cell.child(value),
    };
    div()
        .id(id)
        .flex()
        .flex_row()
        .items_start()
        .gap(px(8.))
        .min_w_0()
        .child(
            div()
                .w(LABEL_WIDTH)
                .flex_none()
                .text_color(theme::color(palette.fg_1))
                .child(fact.label()),
        )
        .child(cell)
}
