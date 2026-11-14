//! Host management: the Hosts panel the sidebar shows in place of its session list while its Hosts control is pressed,
//! and the per-host rules behind it.
//!
//! SPEC.md, "Session list" (the Hosts control, host rows, the update button and Too new) and "Topology", ported from the
//! old app's `hosts.rs` (`HostsPanel`, `HostRow`, the phase vocabulary) and `provisioning.rs`
//! (`ProvisioningMenuState::offered`) and the stylesheet's `.hosts-heading` and `.host-row` rules at reference commit
//! `24bc0580`.
//!
//! ## Entry point
//!
//! [`HostsPanel`] is the view. The sidebar makes one with `cx.new(HostsPanel::new)` and keeps it for the window's life,
//! drawing it as a child in place of the session list's column while Hosts is pressed and not drawing it otherwise.
//! Keeping the one instance across the swap is what keeps the Details choice (SPEC.md: opening or closing Hosts "keeps
//! the current ... details choice"); nothing in the panel needs to run while it is hidden, since everything it shows is
//! read from the store when it draws. It fills the box it is given and scrolls its rows inside it, so the box needs a
//! definite height.
//!
//! ## What it shows
//!
//! A heading band with the host count ("Hosts 3", read out as "3 hosts") and the Details switch, then one row per host
//! in the registry's order, session-owned hosts included (SPEC.md: they "participate in ordinary host management"). A
//! row is the host's identity mark, its name (the alias, else the helm's display name, "Local (this machine)" for the
//! local host under the registry's own name; the destination behind an alias shows in the details), and its phase dot
//! and words ([`phase`]). Under it, while the row is open, the details: the destination behind an alias, what the helm
//! knows about the phase (for a connected host its version, identity and session count) and the remedy where waiting
//! will not help. The last refusal of work on the host shows under the row whether it is open or not.
//!
//! A row is open while the Details switch is on, or while the row opens itself ([`disclosed`]): its last provisioning
//! run failed, the last work on it was refused, or it is the helm's own machine with no supervisor running. The switch
//! is the user's alone: nothing else turns it on, and it is not saved anywhere (SPEC.md: "its unpersisted details
//! switch"). The old app turned the switch on for a failed local setup; the port opens that row instead.
//!
//! A host list never read says "Loading hosts…"; a failed first read says so in red; a failed refresh keeps the rows it
//! had and says above them that they are the last state read. The panel never invents a host.
//!
//! The helm's own machine with no supervisor running reads "Unreachable, retrying" and opens its own row to say no
//! supervisor is running there and to show the helm's own words for what to do (the command its dial failure names),
//! so it never reads as a phantom unreachable host (SPEC.md, "Topology"). It offers no install ([`offers`]). The old
//! app also probed the machine for the helm's `farhelm helm setup` handoff; the migration's settings-and-hosts area
//! decided on 2026-11-14 that the row shows the pushed remedy instead, and no probe is planned.
//!
//! ## Not drawn yet
//!
//! NOTE: Each arrives with the piece of the port that gives it something to do: Update all and the add button in the
//! heading, the row's actions toggle and the menu it opens, the inline `↑ Update` button, setup and Update progress
//! (the inline status, the step list, the one-off opening of a row for a run and its fold). The toggle's place at the
//! row's end is kept empty, at its width, so the phase words sit where they will sit beside it, and the heading keeps
//! its old height. [`offers::offers`] already says what the menu will offer.

pub(crate) mod offers;
mod phase;

#[cfg(test)]
pub(crate) mod tests;

use farhelm_proto::provisioning::ProvisioningView;
use farhelm_proto::ui_channel::{Availability, Collection, HostView};
use gpui::{
    AnyElement, Context, FontWeight, InteractiveElement, IntoElement, ParentElement, Render, Role,
    SharedString, StatefulInteractiveElement, Styled, Window, div, px,
};

use crate::assets;
use crate::components::{Checkbox, HoverHelp, StatusMark};
use crate::host_text::{self, HostSentence, HostText};
use crate::operations::{Failure, OpKey};
use crate::render_count;
use crate::session;
use crate::session_list::host_rail::{display_name, host_mark};
use crate::theme::{self, Palette, Token, metrics};
use offers::{HostKind, Locality};
use phase::{Kind, Part, Tone};

/// `.hosts-heading`'s padding (vertical then horizontal) and the gap between its parts.
const HEADING_PAD: (f32, f32) = (4., 10.);
const HEADING_GAP: f32 = 8.;

/// The heading's height in the old app, where Update all and the 24-pixel add button set it: kept until they arrive, so
/// the rows do not move when they do.
const HEADING_HEIGHT: f32 = 32.;

/// `.hosts-heading .host-count`'s `gap: 1ch`: the width of a digit at the label's 13 pixels.
const COUNT_GAP: f32 = 7.;

/// `.host-row-main`: its height (a 32-pixel content box inside 4 pixels of padding above and below), its left padding,
/// and the gap between its parts.
const ROW_HEIGHT: f32 = 40.;
const ROW_PAD: (f32, f32) = (4., 12.);
const ROW_GAP: f32 = 6.;

/// `.host-kind-icon` in a row: the identity mark's size.
const MARK: f32 = 12.;

/// `.host-name`'s `min-width: 4em`, at its 13 pixels: the floor a long name gives way to before the phase words do.
const NAME_FLOOR: f32 = 52.;

/// `.host-status`'s gap between the dot and the words.
const STATUS_GAP: f32 = 5.;

/// The actions toggle's width at the row's end (`.host-row-menu`: the ⋯ at 14 pixels inside 5 pixels of padding a
/// side) and its right margin. Kept empty until the toggle is drawn.
const TOGGLE_WIDTH: f32 = 24.;
const TOGGLE_MARGIN: f32 = 2.;

/// `.host-detail` and its siblings: their side padding, the pixel above each line, and their line height (the old
/// `normal` line height of 12-pixel Inter, as the reference screens measure it).
const DETAIL_PAD_X: f32 = 12.;
const DETAIL_PAD_TOP: f32 = 1.;
const DETAIL_LINE: f32 = 15.;

/// The space under an open row's last line, before its border, as the reference screens show it (the old stylesheet
/// has no rule for it: it fell out of the old page's line boxes).
const DETAILS_PAD_BOTTOM: f32 = 6.;

/// `.host-error`'s padding above.
const ERROR_PAD_TOP: f32 = 2.;

/// The Hosts panel. See the module docs.
pub struct HostsPanel {
    /// The Details switch: whether every row is open. Starts off and is never saved.
    details: bool,
}

impl HostsPanel {
    /// A panel with Details off. It reads everything else from the store while it draws, so it can be made before the
    /// connection or the first snapshot and needs nothing from them.
    pub fn new(_cx: &mut Context<Self>) -> Self {
        HostsPanel { details: false }
    }

    /// Turn Details on or off, as the switch does.
    fn set_details(&mut self, on: bool, cx: &mut Context<Self>) {
        self.details = on;
        cx.notify();
    }

    /// Turn Details on or off from a test or a golden scene, as a toggle of the switch would.
    #[cfg(test)]
    pub(crate) fn show_details(&mut self, on: bool, cx: &mut Context<Self>) {
        self.set_details(on, cx);
    }
}

/// Whether `host`'s row is open: the Details switch is on, the host's last provisioning run failed (`progress`), the
/// last work on the host was refused (`refused`), or the host is the helm's own machine with no supervisor running. The
/// last three open the row by themselves, so a failure always shows its evidence and remedies, and the local host
/// without a supervisor shows what to do rather than reading as a phantom unreachable host (SPEC.md, "Topology"). Each
/// closes it again once it no longer holds: a new run or new work on the host replaces the first two, and a supervisor
/// starting ends the last.
pub(crate) fn disclosed(
    details: bool,
    host: &HostView,
    progress: Option<&ProvisioningView>,
    refused: bool,
) -> bool {
    details
        || offers::failed_operation(progress).is_some()
        || refused
        || offers::needs_local_setup(host)
}

/// What one row is drawn from, borrowed from the store and the operation store for the frame being drawn.
struct RowInput<'a> {
    host: &'a HostView,
    progress: Option<&'a ProvisioningView>,
    refusal: Option<&'a Failure>,
}

impl Render for HostsPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        render_count::record("hosts-panel", cx);
        let palette = theme::palette(cx);
        let weak = cx.weak_entity();
        let operations = session::current_operations(cx);
        let mut status = None;
        let mut drawn: Vec<AnyElement> = Vec::new();
        // `None` until the hosts collection arrived: the count is not known yet.
        let mut count = None;
        if let Some(store) = session::current_store(cx) {
            let collections = store.read(cx).collections();
            if let Some(meta) = collections.meta(Collection::Hosts) {
                let hosts = collections.ordered::<HostView>();
                count = Some(hosts.len());
                status = match &meta.availability {
                    Availability::Available => None,
                    // A failed read is no evidence that a host went: the rows it had stay, under a line that says so.
                    Availability::Unavailable { error } if !hosts.is_empty() => Some((
                        "Showing the last state this client read; the refresh failed: ",
                        HostText::from_host(error.as_str()),
                    )),
                    Availability::Unavailable { error } => Some((
                        "The hosts list could not be loaded: ",
                        HostText::from_host(error.as_str()),
                    )),
                };
                let operations = operations.as_ref().map(|operations| operations.read(cx));
                // A duplicate names the entry that holds the same machine by the name its own row shows.
                let twin = |id: i64| {
                    hosts
                        .iter()
                        .find(|host| host.id == id)
                        .map(|host| display_name(host))
                };
                drawn = hosts
                    .iter()
                    .map(|host| {
                        let key = HostView::key(host.id);
                        let input = RowInput {
                            host,
                            progress: collections.get::<ProvisioningView>(&key),
                            refusal: operations
                                .and_then(|operations| operations.failure(&OpKey::host(&key))),
                        };
                        self.row(&input, &twin, palette)
                    })
                    .collect();
            }
        }

        let heading = div()
            .flex()
            .flex_row()
            .flex_wrap()
            .items_center()
            .gap(px(HEADING_GAP))
            .min_h(px(HEADING_HEIGHT))
            .py(px(HEADING_PAD.0))
            .px(px(HEADING_PAD.1))
            .bg(theme::color(palette.bg_2))
            .child(count_heading(count, palette))
            .child(
                div().ml_auto().py(px(2.)).text_size(metrics::TEXT_SM).child(
                    Checkbox::new(
                        "host-details-toggle",
                        "Details",
                        HoverHelp::tooltip(
                            "Details: show every host's version, identity, sessions and diagnostics",
                        ),
                        Role::Switch,
                    )
                    .switch()
                    .inline()
                    .checked(self.details)
                    .on_toggle(move |on, _, cx| {
                        let _ = weak.update(cx, |panel, cx| panel.set_details(on, cx));
                    }),
                ),
            );
        let status = match (status, count) {
            (Some((words, error)), _) => Some(
                div()
                    .py(px(4.))
                    .px(px(10.))
                    .text_size(metrics::TEXT_SM)
                    .text_color(theme::color(palette.danger))
                    .child(
                        HostSentence::new("hosts-load-error")
                            .text(words)
                            .host(&error),
                    )
                    .into_any_element(),
            ),
            (None, None) => Some(
                div()
                    .id("hosts-status")
                    .role(Role::Status)
                    .aria_label("Loading hosts…")
                    .py(px(4.))
                    .px(px(10.))
                    .text_size(metrics::TEXT_SM)
                    .text_color(theme::color(palette.fg_1))
                    .child("Loading hosts…")
                    .into_any_element(),
            ),
            (None, Some(_)) => None,
        };
        div()
            .id("hosts-panel")
            // Tests and the bridge find the panel by this; it records nothing outside test builds.
            .debug_selector(|| "hosts-panel".into())
            .role(Role::Region)
            .aria_label("Host management")
            .size_full()
            .flex()
            .flex_col()
            .min_h_0()
            .child(heading)
            .children(status)
            .child(
                div()
                    .id("host-list")
                    .role(Role::List)
                    .aria_label("Hosts")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .child(div().flex().flex_col().children(drawn)),
            )
    }
}

impl HostsPanel {
    /// One host's row (`.host-row`): its header line, then its details while it is open, then the last refusal of work
    /// on it, which shows open or not. `twin` names a host by id for a duplicate's details.
    fn row(
        &self,
        input: &RowInput,
        twin: &impl Fn(i64) -> Option<HostText>,
        palette: &Palette,
    ) -> AnyElement {
        let host = input.host;
        let id = host.id;
        let name = display_name(host);
        let open = disclosed(self.details, host, input.progress, input.refusal.is_some());
        let main = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(ROW_GAP))
            .min_h(px(ROW_HEIGHT))
            .py(px(ROW_PAD.0))
            .pl(px(ROW_PAD.1))
            .children(
                HostKind::of(host)
                    .locality()
                    .map(|locality| identity_mark(host, locality, palette)),
            )
            .child(
                host_text::element(&name)
                    .flex_1()
                    .min_w(px(NAME_FLOOR))
                    .truncate()
                    .text_size(metrics::TEXT_MD)
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme::color(palette.fg_0)),
            )
            .child(phase_status(host, palette))
            .child(div().flex_none().w(px(TOGGLE_WIDTH)).mr(px(TOGGLE_MARGIN)));
        let lines: Vec<AnyElement> = if open {
            phase::detail_lines(host, twin)
                .into_iter()
                .enumerate()
                .map(|(n, (kind, line))| {
                    detail_line(format!("host-detail-{id}-{n}"), kind, &line, palette)
                })
                .collect()
        } else {
            Vec::new()
        };
        let refusal = input.refusal.map(|refusal| {
            div()
                .pt(px(ERROR_PAD_TOP))
                .px(px(DETAIL_PAD_X))
                .text_size(metrics::TEXT_SM)
                .text_color(theme::color(palette.danger))
                .child(
                    HostSentence::new(SharedString::from(format!("host-error-{id}")))
                        .host(&refusal.message),
                )
        });
        // The lines under the header (`.host-row-details`): the details while the row is open, then the refusal, which
        // shows open or not. Nothing at all when there is neither, so a closed row is its header alone.
        let under = (!lines.is_empty() || refusal.is_some()).then(|| {
            div()
                .pb(px(DETAILS_PAD_BOTTOM))
                .children(lines)
                .children(refusal)
        });
        div()
            .id(SharedString::from(format!("host-row-{id}")))
            .role(Role::ListItem)
            .aria_label(
                HostSentence::new("host-row-name")
                    .host(&name)
                    .accessible_name(),
            )
            .flex()
            .flex_col()
            .border_b_1()
            .border_color(theme::color(palette.border_row))
            .hover(|row| row.bg(theme::color(palette.bg_2)))
            .child(main)
            .children(under)
            .into_any_element()
    }
}

/// The heading's count (`.host-count`): "Hosts" in the heading's weight with the number after it in the quiet ink,
/// read out as "3 hosts" (or "1 host"), the way the old markup kept the number first for screen readers and only drew
/// it second. Before the host list arrives, "Hosts" alone.
fn count_heading(count: Option<usize>, palette: &Palette) -> impl IntoElement {
    let (label, name) = match count {
        Some(1) => ("Host", "1 host".to_owned()),
        Some(n) => ("Hosts", format!("{n} hosts")),
        None => ("Hosts", "Hosts".to_owned()),
    };
    div()
        .id("host-count")
        .role(Role::Heading)
        .aria_label(name)
        .flex()
        .flex_row()
        .items_baseline()
        .gap(px(COUNT_GAP))
        .child(
            div()
                .text_size(metrics::TEXT_MD)
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme::color(palette.fg_0))
                .child(label),
        )
        .children(count.map(|n| {
            div()
                .text_size(metrics::TEXT_SM)
                .text_color(theme::color(palette.fg_1))
                .child(n.to_string())
        }))
}

/// A host's identity mark in its row: its chosen shape and color at 12 pixels, a status named "Local" or "Remote" for
/// assistive technology, with hover help saying what that means. It never gives way to a long name.
fn identity_mark(host: &HostView, locality: Locality, palette: &Palette) -> AnyElement {
    let (word, help) = match locality {
        Locality::Local => ("Local", "Local: on this machine, where the helm runs"),
        Locality::Remote => ("Remote", "Remote: on another machine, reached over SSH"),
    };
    div()
        .flex_none()
        .child(
            StatusMark::new(
                SharedString::from(format!("host-mark-{}", host.id)),
                word,
                HoverHelp::tooltip(help),
                Role::Image,
                word,
            )
            .content(host_mark(host.icon, host.color, px(MARK), palette)),
        )
        .into_any_element()
}

/// A host's phase (`.host-status`): the dot and the words in the phase's tone, the words cut short before the toggle's
/// place leaves the row. Named by its words; its hover is the words, or for a host running a newer Farhelm both
/// versions and the remedy, which is then also its description, as the old status described only Too new.
fn phase_status(host: &HostView, palette: &Palette) -> AnyElement {
    let words = phase::words(&host.state);
    let ink = tone_ink(phase::tone(&host.state), palette);
    // `min_w_0` lets the status shrink below its words, so they are cut short rather than pushed out of the row.
    div()
        .min_w_0()
        .child(
            StatusMark::new(
                SharedString::from(format!("host-status-{}", host.id)),
                words,
                HoverHelp::tooltip(phase::hover(&host.state)),
                Role::Status,
                words,
            )
            .ink(ink)
            .described(offers::runs_newer_version(&host.state))
            .content(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(STATUS_GAP))
                    .whitespace_nowrap()
                    .child(
                        div()
                            .flex_none()
                            .size(metrics::STATUS_DOT)
                            .rounded_full()
                            .bg(theme::color(ink)),
                    )
                    .child(div().min_w_0().truncate().child(words)),
            ),
        )
        .into_any_element()
}

/// The ink of a phase's tone (the old `.host-status.<class>` colors).
fn tone_ink(tone: Tone, palette: &Palette) -> Token {
    match tone {
        Tone::Connected => palette.ok,
        Tone::Advisory => palette.warn,
        Tone::Pending | Tone::Quiet => palette.fg_1,
        Tone::Attention => palette.danger_strong,
    }
}

/// One detail line (`.host-detail`, `.host-destination-detail`, `.host-remedy`): 12-pixel text, the remedy in the
/// ordinary ink and the rest in the quiet one, each host value its own text element (build, identity and destination
/// in the monospace font), and the whole line one labelled node for assistive technology.
///
/// NOTE: The parts wrap as whole pieces, as every sentence mixing the app's words with host text does
/// ([`HostSentence`]): a long value starts on a line of its own and wraps inside itself, where the old page broke it
/// mid-run after the words before it.
fn detail_line(id: String, kind: Kind, line: &phase::Line, palette: &Palette) -> AnyElement {
    let ink = match kind {
        Kind::Evidence => palette.fg_1,
        Kind::Remedy => palette.fg_0,
    };
    div()
        .id(SharedString::from(id))
        .role(Role::Label)
        .aria_label(line.accessible_name())
        .flex()
        .flex_row()
        .flex_wrap()
        .pt(px(DETAIL_PAD_TOP))
        .px(px(DETAIL_PAD_X))
        .text_size(metrics::TEXT_SM)
        .line_height(px(DETAIL_LINE))
        .text_color(theme::color(ink))
        .children(line.0.iter().map(|part| -> AnyElement {
            match part {
                Part::Text(words) => div().min_w_0().child(words.clone()).into_any_element(),
                Part::Host(value) => host_text::element(value).min_w_0().into_any_element(),
                Part::Mono(value) => host_text::element(value)
                    .min_w_0()
                    .font_family(assets::TERMINAL_FONT)
                    .into_any_element(),
            }
        }))
        .into_any_element()
}
