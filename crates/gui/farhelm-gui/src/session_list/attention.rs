//! The Hosts attention list: the hosts that need something, one row each, in the list's column above its heading, so a
//! problem, a run going on or an update is visible without opening Hosts.
//!
//! SPEC.md, "Session list" (the attention list's paragraphs, and its place: "a compact host list sits below the app bar
//! and above the session list's title and status filter ... absent when nothing needs attention and while Hosts is
//! open"). Ported from the old app's `hosts/attention_list.rs` (`classify`, `problem_words`, `layout_key`,
//! `HostAttentionList`, `AttentionHostRow`), `hosts/updates.rs` (`UpdateDismissal`), the attention parts of
//! `list/view.rs` (dismissal, Details, the menu-retirement key, the placement) and the stylesheet's `.host-attention-*`
//! rules, at reference commit `24bc0580`. Here's the TLDR of its rules:
//!
//! - **One row per host, one reason each** ([`classify`]): work going on (a setup, Update or uninstall run, or its
//!   requests out) over a problem, over an update, so a host whose Update loses its connection stays one working row.
//!   Rows are grouped problems, then working hosts, then updates, each group in the registry's order (the rail's order;
//!   a host a session owns has no avatar but keeps its row, as SPEC.md has "the attention area retain ordinary
//!   diagnostics and controls" for it). Every fact is [`crate::hosts::status`]'s, read per frame: the update badge's
//!   test, the work going on, and the setup or Update warning.
//! - **Problem rows** ([`problem_words`]) say what is wrong and offer Details; **working rows** say "Setting up",
//!   "Updating" or "Uninstalling" and the current step (or "Deleting" for an environment being destroyed, below),
//!   over a progress bar, with no button; **update rows** offer Update, and a required one says sessions stay
//!   unreachable until it is updated. Details, or a press anywhere on a row outside its button, shows Hosts through
//!   [`SessionList::show_hosts`] (which acknowledges the hosts' warnings) and asks the panel to bring that host's row
//!   into view with its menu toggle focused ([`crate::hosts::HostsPanel::reveal`]); the session filter stays as it is.
//! - **The header** counts the rows ("N hosts need attention") and offers Update all only while more than one row
//!   offers Update, Always update while "Update remote hosts automatically" reads off and some row offers Update, and
//!   the close control while some row is an optional update. Each acts on exactly the hosts drawn
//!   ([`crate::hosts::update::start_all`], [`crate::hosts::status::always_update`]). Always update's failed save shows
//!   under the header with its Try again ([`crate::hosts::status::auto_update_failed`]), and never keeps an otherwise
//!   empty list visible.
//! - **Dismissal** ([`Dismissal`]) hides the optional update rows only, until a host outside the set of hosts with an
//!   update at the dismissal gains one, which forgets the dismissal for good. It belongs to the UI session, so a reset
//!   shows the reminders again.
//! - **While a session launch is in flight**, Update, Update all and Always update are drawn unavailable, still
//!   focusable, with "Wait for the pending launch to finish" as their hover help and description, and a press sends
//!   nothing ([`Shown::launch_pending`]). An Update would be refused then anyway ([`crate::hosts::update`]), and its
//!   reason goes to the Hosts panel's line, which is not drawn while this list is. The old app queued such an Update
//!   until the launch was done; the port refuses it visibly instead (decided by the area's owner).
//! - **Menus below the list.** A change of what the list draws moves everything under it, so it closes the rows' menus
//!   and the marked menu ([`SessionList::attention_moved`]): rows coming or going, their visible words, and the
//!   header's buttons or failure line appearing or going ([`LayoutKey`]). A run's step and step count are not in the
//!   key: they repaint inside a row whose height never changes, so a progress tick keeps an open menu open.
//! - **The working row's hover** is the host progress card ([`crate::hosts::progress_card`]), beside the sidebar,
//!   made only while it shows; before the run's steps arrive the row has its plain hover help. The whole row is the one
//!   control that carries it, so moving within the row keeps one card.
//!
//! The rows scroll inside the list, which is at most 280 pixels or 45% of the column high (the old `max-height:
//! min(280px, 45%)`), so many hosts never push the sessions away; the body is a focus coordinator scroll region, so
//! Tab onto a row below its fold scrolls it into view.
//!
//! NOTE: Deviations from the old list. The old dismissal also ended when the helm's build changed; the UI channel
//! reports no helm build (the desktop app runs the helm it embeds), so a dismissal lasts its UI session instead, and a
//! helm restarted on another build starts a new UI session anyway. A row's open control fills the row up to its button
//! and draws the list rows' hover fill, where the old one had no fill. The update actions wait visibly for a launch in
//! flight (above), where the old ones queued the Update.
//!
//! Keyboard focus on a working row's open control shows the same card (SPEC.md), since the layer manager shows a
//! control's help on keyboard focus as well as on hover, beside the sidebar where the help sits there.
//!
//! **Sandbox and container hosts** get rows of their own, ported from the same `classify` and `problem_words`. This
//! list is how a user finds a stuck environment, since a host a session owns has no rail avatar to warn them:
//!
//! - An environment being destroyed (`Deleting` with no failure: a sealed environment, or one deleted from Hosts) is a
//!   working row, "Deleting", with an empty bar, no button and always the plain hover help (it never has a run, so
//!   never a progress card). As a reason it ranks after live work and above any problem, so a warned or
//!   credential-less machine that is going away is still only "Deleting"; it is drawn in the working group.
//! - A failed deletion ("Deleting failed") and a stopped environment ("Stopped") are problem rows in their phase's own
//!   words ([`phase::words`]), and a suspended one whose resume failed reads "Resume failed". An environment that is
//!   merely suspended, suspending, resuming or starting has no row: that is routine, and SPEC.md lists it among the
//!   hosts that need none.
//! - "Credentials not delivered" ([`phase::credential_warning`]) and "Tunnels failed" ([`phase::tunnels_failed`],
//!   which counts only while the host reads connected, so stale forwarding health of a host that is away claims
//!   nothing) follow the phase's words on any row, before the setup warning, joined by "; " as the old line was. They
//!   are the same tests the Hosts row and the rail read, so the three cannot disagree about one host.
//!
//! These rows offer Details like every problem row, and nothing else: the old list never had an inline Retry or Resume.
//! Hosts offers Retry for a failed deletion, in the host's menu (`hosts/remedies.rs`).
//!
//! NOTE: SPEC.md, "Sandbox sessions", has the attention area show a failed deletion "with the reason and **Retry**".
//! That sentence was already there at the old app's reference commit, whose row offered only Details, so the old app
//! is its reference reading (decided by the GPUI migration's orchestrator on 2026-11-16): the row shows the failure and
//! leads to Hosts, which shows the reason and Retry, with no Retry in the row. SPEC.md's wording is to be revised to
//! say so.
//!
//! NOTE: Hosts' Resume is not ported yet (its menu's NOTE), so Details on a "Resume failed" or "Stopped" row reaches no
//! Resume there for now; the session's own card in the session pane offers one.

#[cfg(test)]
mod tests;

use std::collections::HashSet;

use farhelm_proto::ui_channel::{HostStateView, HostView};
use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, AnyView, App, AppContext as _, Context, FontWeight, InteractiveElement,
    IntoElement, ParentElement, Role, ScrollHandle, SharedString, StatefulInteractiveElement,
    Styled, WeakEntity, Window, div, px, relative,
};

use super::SessionList;
use super::host_rail::identity;
use super::menu::LAUNCH_PENDING;
use crate::assets::Icon;
use crate::components::{Button, HoverHelp, IconButton, ListRow, Tier};
use crate::focus::{self, Mark};
use crate::host_text::{self, HostSentence, HostText};
use crate::hosts::identity::{host_mark, phase, short_name};
use crate::hosts::offers::runs_newer_version;
use crate::hosts::status::{self, HostStatus, Steps, Urgency, host_status};
use crate::hosts::{progress_card, update};
use crate::session;
use crate::theme::{self, Palette, metrics};

/// `.host-attention-list`'s `max-height: min(280px, 45%)`: the list's cap, in pixels and as a share of the column.
const MAX_HEIGHT: f32 = 280.;
const MAX_SHARE: f32 = 0.45;

/// `.host-attention-list`'s margin above and at its sides.
const MARGIN: f32 = 8.;

/// `.host-attention-header`'s padding and the gap between its parts.
const HEADER_PAD: f32 = 8.;
const HEADER_GAP: f32 = 6.;

/// `.host-attention-list`'s `line-height: 1.4` at its 12 pixels.
const LINE_HEIGHT: f32 = 16.8;

/// `.host-attention-open .host-kind-icon`: the mark's size, the pixel it sits down from the name's top, and its gap to
/// the words.
const MARK: f32 = 16.;
const MARK_DROP: f32 = 1.;
const MARK_GAP: f32 = 6.;

/// `.host-attention-row`'s right padding, which the open control leaves to the row's button and the bar.
const ROW_PAD_RIGHT: f32 = 8.;

/// `.host-attention-progress`'s height.
const BAR_HEIGHT: f32 = 5.;

/// The progress card's width (`.farhelm-tooltip[data-kind="card"]`).
const CARD_WIDTH: f32 = 310.;

/// What a required update row says under its state, the old `.host-attention-required` words.
const REQUIRED: &str = "Sessions stay unreachable until this host is updated.";

// ---- Which hosts need attention -------------------------------------------------------------------------------------

/// Why a host has a row; each host has at most one, by [`classify`]'s precedence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Kind {
    /// Something is wrong that Details shows (the phase's own trouble, an environment's undelivered credentials or
    /// failed forwards, or an unacknowledged setup or Update warning).
    Problem,
    /// A setup, Update or uninstall is going on, or the host's sandbox or container is being destroyed.
    Working,
    /// The host has an update by the rail badge's test; `required` when its sessions cannot be reached until then.
    Update { required: bool },
}

impl Kind {
    /// The group's place in the list: problems, then working hosts, then updates.
    fn group(self) -> u8 {
        match self {
            Kind::Problem => 0,
            Kind::Working => 1,
            Kind::Update { .. } => 2,
        }
    }
}

/// One host's row as one frame draws it: a snapshot, never looked up again when a button is pressed.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Row {
    pub(super) host: HostView,
    pub(super) kind: Kind,
    /// The state, as the row's second line says it before any step: a working row's verb, a problem's words, or
    /// "Update available" / "Update required".
    pub(super) words: String,
    /// A working row's run, once a run shows; `None` while only its requests are out (an empty bar) and on other rows.
    pub(super) steps: Option<Steps>,
}

/// The rows for `hosts` (each with what [`crate::hosts::status`] reads about it, in the registry's order), grouped and
/// in order, with the optional update rows left out while `dismissed`. A host appears once, for its first reason of:
/// work going on, a problem, an update (SPEC.md: "Live setup, update or uninstall progress takes precedence over a
/// problem, which takes precedence over an available update"). An environment being destroyed counts as work going on:
/// as a reason it ranks after a live run and above a problem, as the old `classify` ranked it, and its row is drawn in
/// the working group. Pure, so the rules are tested without a window.
pub(super) fn classify<'a>(
    hosts: impl IntoIterator<Item = (&'a HostView, &'a HostStatus)>,
    dismissed: bool,
) -> Vec<Row> {
    let mut rows: Vec<Row> = hosts
        .into_iter()
        .filter_map(|(host, status)| {
            let (kind, words, steps) = if let Some(live) = &status.live {
                (
                    Kind::Working,
                    live.activity.verb().to_owned(),
                    live.steps.clone(),
                )
            } else if matches!(host.state, HostStateView::Deleting { failure: None }) {
                // The destruction has no steps to show, so the bar stays empty, as the old row's did.
                (Kind::Working, "Deleting".to_owned(), None)
            } else if let Some(words) = problem_words(host, status) {
                (Kind::Problem, words, None)
            } else {
                let required = status.update? == Urgency::Required;
                if !required && dismissed {
                    return None;
                }
                let words = if required {
                    "Update required"
                } else {
                    "Update available"
                };
                (Kind::Update { required }, words.to_owned(), None)
            };
            Some(Row {
                host: host.clone(),
                kind,
                words,
                steps,
            })
        })
        .collect();
    // A stable sort keeps the registry's order inside each group, optional and required updates mixed as listed.
    rows.sort_by_key(|row| row.kind.group());
    rows
}

/// What is wrong with `host`, or `None` when nothing is: its phase's words for a phase that waits for someone (SPEC.md:
/// "Unreachable (worded **Unreachable, retrying**), Identity mismatch, Identity unverified, Duplicate, Retired"), a
/// protocol skew where the host is the newer side ("Too new; update the helm") or has no Update on offer to fix it,
/// an environment's trouble ("Stopped", "Deleting failed", "Resume failed"), then "Credentials not delivered" and
/// "Tunnels failed" for a sandbox or container whose helm says so, and last "Setup or update needs attention" for an
/// unacknowledged warning, every part that applies joined by "; ". Connecting, a connected host (also one on a newer
/// build, which is usable), a skew whose required Update is on offer (an update row says it), and an environment
/// suspended, changing or being deleted without a failure are not problems in themselves.
pub(super) fn problem_words(host: &HostView, status: &HostStatus) -> Option<String> {
    // Exhaustive, so a new phase decides here whether it needs a row.
    let phase = match &host.state {
        HostStateView::Unreachable { .. }
        | HostStateView::IdentityMismatch { .. }
        | HostStateView::IdentityUnverified { .. }
        | HostStateView::Duplicate { .. }
        | HostStateView::Retired { .. }
        | HostStateView::Stopped { .. }
        | HostStateView::Deleting { failure: Some(_) } => Some(phase::words(&host.state)),
        HostStateView::Suspended {
            resume_failed: Some(_),
            ..
        } => Some("Resume failed"),
        HostStateView::VersionSkew { .. } if runs_newer_version(&host.state) => {
            Some("Too new; update the helm")
        }
        HostStateView::VersionSkew { .. } if status.update != Some(Urgency::Required) => {
            Some(phase::words(&host.state))
        }
        HostStateView::VersionSkew { .. }
        | HostStateView::Connecting { .. }
        | HostStateView::Connected { .. } => None,
        // Routine lifecycle: a suspension the user or the provider chose, a change on its way, and a destruction that
        // `classify` has already made a working row.
        HostStateView::TensorlakeChanging { .. }
        | HostStateView::Suspended {
            resume_failed: None,
            ..
        }
        | HostStateView::Deleting { failure: None } => None,
    };
    // Delivery and forwarding fail independently of the phase, so they are added to its words rather than replacing
    // them, as the old line kept a connection failure beside them.
    let credentials = phase::credential_warning(host).map(|_| "Credentials not delivered");
    let tunnels = phase::tunnels_failed(host).then_some("Tunnels failed");
    let warning = status.warned.then_some("Setup or update needs attention");
    let words: Vec<&str> = phase
        .into_iter()
        .chain(credentials)
        .chain(tunnels)
        .chain(warning)
        .collect();
    (!words.is_empty()).then(|| words.join("; "))
}

/// The optional update reminders the user closed: the hosts that had an update when they did (optional and required
/// alike, as the old `UpdateDismissal` kept them). A host that loses its update during a run and regains it is not new.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct Dismissal {
    hosts: HashSet<i64>,
}

impl Dismissal {
    /// Dismiss the reminders of `updates`, the hosts with an update now.
    pub(super) fn new(updates: HashSet<i64>) -> Dismissal {
        Dismissal { hosts: updates }
    }

    /// Whether `updates`, the hosts with an update now, include one the dismissal did not see, which ends it (SPEC.md:
    /// hidden "until a host outside the dismissed update set gains an update").
    pub(super) fn ended_by(&self, updates: &HashSet<i64>) -> bool {
        !updates.is_subset(&self.hosts)
    }
}

/// What the list keeps for one UI session: the dismissal, the layout the last frame drew, which the next one is
/// compared with to close the menus below the list when it changes, and the hosts that frame read with an update.
#[derive(Default)]
pub(super) struct Memory {
    dismissal: Option<Dismissal>,
    layout: Option<LayoutKey>,
    /// The hosts with an update as the last frame read them: what the close control dismisses, so a dismissal records
    /// exactly the set that [`Dismissal::ended_by`] is later compared with.
    updates: HashSet<i64>,
}

/// What of the list moves the session list below it when it changes: each row's host, kind, short name and state
/// words, and whether Always update and its failure line show (Update all and the close control follow the rows).
/// Steps, counts and the bar's fill are left out: they repaint inside rows of fixed height (the old `layout_key`).
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct LayoutKey {
    rows: Vec<(i64, Kind, HostText, String)>,
    always_update: bool,
    failed: bool,
}

/// The list one frame draws: its rows (never empty), and the preference's state the header reads.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Shown {
    pub(super) rows: Vec<Row>,
    /// Whether "Update remote hosts automatically" reads on, which withdraws Always update.
    pub(super) auto_updates_on: bool,
    /// Whether Always update's last save failed while the preference still reads off.
    pub(super) failed: bool,
    /// Whether a session launch is in flight, which holds Update, Update all and Always update back: the rule is the one
    /// [`crate::hosts::update::start`] refuses by ([`crate::hosts::launch_in_flight`]), and the buttons say so in the
    /// launcher triggers' words for a pending launch ([`super::menu::LAUNCH_PENDING`], [`waiting`]). Not part of the
    /// [`LayoutKey`]: the buttons keep their place and size.
    pub(super) launch_pending: bool,
}

impl Shown {
    /// The hosts whose rows offer Update, in the order drawn: Update all's and Always update's targets.
    fn updates(&self) -> Vec<i64> {
        self.rows
            .iter()
            .filter(|row| matches!(row.kind, Kind::Update { .. }))
            .map(|row| row.host.id)
            .collect()
    }

    /// Whether Always update shows: the preference reads off and some row offers Update.
    fn always_update(&self) -> bool {
        !self.auto_updates_on && !self.updates().is_empty()
    }

    /// This frame's [`LayoutKey`].
    pub(super) fn layout_key(&self) -> LayoutKey {
        LayoutKey {
            rows: self
                .rows
                .iter()
                .map(|row| {
                    (
                        row.host.id,
                        row.kind,
                        short_name(&row.host),
                        row.words.clone(),
                    )
                })
                .collect(),
            always_update: self.always_update(),
            failed: self.failed,
        }
    }
}

// ---- The list's frame and its actions -------------------------------------------------------------------------------

impl SessionList {
    /// The attention list for this frame, or `None` when it is not drawn (Hosts shows, or nothing needs attention).
    ///
    /// Every frame reads the hosts and ends a dismissal a new update outdates, whether or not the list is drawn, and
    /// compares what it draws with the last frame's [`LayoutKey`] (an absent list draws the empty one): a change closes
    /// the menus below the list once the frame is done ([`SessionList::attention_moved`]), since a layer is not
    /// closed while the window draws.
    pub(super) fn attention_frame(
        &mut self,
        hosts_shown: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let shown = self.read_attention(cx).filter(|_| !hosts_shown);
        let key = shown.as_ref().map(Shown::layout_key).unwrap_or_default();
        let memory = &mut self.current.current(cx).attention;
        if memory.layout.as_ref().is_some_and(|before| *before != key) {
            cx.defer_in(window, |list, window, cx| list.attention_moved(window, cx));
        }
        memory.layout = Some(key);
        let shown = shown?;
        Some(list(
            &shown,
            &cx.weak_entity(),
            &self.attention_scroll,
            theme::palette(cx),
        ))
    }

    /// The rows the current UI session's hosts call for, or `None` when there are none (or no UI session). Ends the
    /// dismissal first when a host outside it has an update now.
    fn read_attention(&mut self, cx: &mut Context<Self>) -> Option<Shown> {
        let store = session::current_store(cx)?;
        let collections = store.read(cx).collections();
        let hosts: Vec<(&HostView, HostStatus)> = collections
            .ordered::<HostView>()
            .into_iter()
            .map(|host| (host, host_status(host, cx)))
            .collect();
        let updates: HashSet<i64> = hosts
            .iter()
            .filter(|(_, status)| status.update.is_some())
            .map(|(host, _)| host.id)
            .collect();
        let memory = &mut self.current.current(cx).attention;
        if memory
            .dismissal
            .as_ref()
            .is_some_and(|dismissal| dismissal.ended_by(&updates))
        {
            memory.dismissal = None;
        }
        memory.updates = updates;
        let rows = classify(
            hosts.iter().map(|(host, status)| (*host, status)),
            memory.dismissal.is_some(),
        );
        (!rows.is_empty()).then(|| Shown {
            rows,
            auto_updates_on: status::auto_updates_on(cx),
            failed: status::auto_update_failed(cx),
            launch_pending: crate::hosts::launch_in_flight(cx),
        })
    }

    /// The attention list's drawn layout changed: close the open row menu and the marked menu, which return focus to
    /// their buttons, as the old list retired the menus whose anchors it moved. Reopened, a menu opens where its button
    /// is drawn now.
    fn attention_moved(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let keys = self.current.current(cx).rows.borrow().opened_rows();
        for key in keys {
            self.close_menu(&key, window, cx);
        }
        self.close_marked_menu(window, cx);
    }

    /// Details on `host`'s row, or a press on the row outside its button, at `mark`: show Hosts the one way it is shown
    /// ([`SessionList::show_hosts`], which acknowledges every warning), then have the panel bring the host's row into
    /// view with its menu toggle focused. The session filter, the sort and the open session stay as they are.
    pub(super) fn show_attention_host(
        &mut self,
        host: i64,
        mark: Mark,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.show_hosts(cx);
        self.hosts_panel
            .update(cx, |panel, cx| panel.reveal(host, &mark, window, cx));
    }

    /// The close control, pressed at `mark`: dismiss the optional update reminders of every host with an update as the
    /// list last read them ([`Memory`]), and hand focus to the Hosts control, since the control that held it goes with
    /// the rows it closed (as the old close control did).
    fn dismiss_updates(&mut self, mark: Mark, window: &mut Window, cx: &mut Context<Self>) {
        let memory = &mut self.current.current(cx).attention;
        memory.dismissal = Some(Dismissal::new(memory.updates.clone()));
        cx.notify();
        focus::focus_control(&self.hosts_focus, &mark, window, cx);
    }
}

// ---- Drawing --------------------------------------------------------------------------------------------------------

/// The list (`.host-attention-list`): the header, Always update's failure line, then the rows in their own scroll
/// region, capped at [`MAX_HEIGHT`] and [`MAX_SHARE`] of the column. `list` is the session list its controls act
/// through, and `scroll` the rows' scroll state, which the session list keeps.
fn list(
    shown: &Shown,
    list: &WeakEntity<SessionList>,
    scroll: &ScrollHandle,
    palette: &Palette,
) -> AnyElement {
    let count = shown.rows.len();
    let heading = if count == 1 {
        "1 host needs attention".to_owned()
    } else {
        format!("{count} hosts need attention")
    };
    let rows = div()
        .id("host-attention-body")
        // Tests find the rows' scrolling part by this; it records nothing outside test builds.
        .debug_selector(|| "host-attention-body".into())
        .flex_1()
        .min_h_0()
        .overflow_y_scroll()
        .child(
            div().flex().flex_col().children(
                shown
                    .rows
                    .iter()
                    .map(|row| row_element(row, shown.launch_pending, list, palette)),
            ),
        );
    // Two boxes for the two caps: the outer one takes at most its share of the column, the inner one at most 280
    // pixels of that, and the rows' region shrinks to what is left under the header.
    div()
        .flex_none()
        .flex()
        .flex_col()
        .min_h_0()
        .max_h(relative(MAX_SHARE))
        .pt(px(MARGIN))
        .px(px(MARGIN))
        .child(
            div()
                .id("host-attention-list")
                .debug_selector(|| "host-attention-list".into())
                .role(Role::Region)
                .aria_label("Hosts needing attention")
                .flex()
                .flex_col()
                .min_h_0()
                .max_h(px(MAX_HEIGHT))
                .border_1()
                .border_color(theme::color(palette.border_nested))
                .rounded(metrics::RADIUS_SM)
                .bg(theme::color(palette.bg_1))
                .text_color(theme::color(palette.fg_0))
                .text_size(metrics::TEXT_SM)
                .line_height(px(LINE_HEIGHT))
                .child(header(shown, heading, list))
                .when(shown.failed, |region| region.child(failure(palette)))
                .child(focus::scroll_region(rows, scroll)),
        )
        .into_any_element()
}

/// `.host-attention-header`: the count, then Update all (more than one row offers Update), Always update (the
/// preference reads off and some row offers Update) and the close control (some row is an optional update).
fn header(shown: &Shown, heading: String, list: &WeakEntity<SessionList>) -> AnyElement {
    let updates = shown.updates();
    let optional = shown
        .rows
        .iter()
        .any(|row| row.kind == Kind::Update { required: false });
    // While a launch is in flight these keep their place, unavailable, and say why ([`Shown::launch_pending`]).
    let pending = shown.launch_pending;
    let help =
        |ready: &'static str| HoverHelp::tooltip(if pending { LAUNCH_PENDING } else { ready });
    let update_all = (updates.len() > 1).then(|| {
        let hosts = updates.clone();
        Button::new(
            "host-attention-update-all",
            "Update all",
            help("Update all: install this helm's version on the listed update hosts"),
            Role::Button,
        )
        .tier(Tier::Neutral)
        .compact()
        .when(pending, |button| {
            button.busy(true).accessible_name(waiting("Update all"))
        })
        .on_click(move |_, window, cx| {
            let mark = Mark::take(cx);
            update::start_all(&hosts, mark, window, cx);
        })
    });
    let always_update = shown.always_update().then(|| {
        Button::new(
            "host-attention-always-update",
            "Always update",
            help(
                "Update the listed hosts now and turn on Update remote hosts automatically: later updates install \
                 without asking. Change this in Settings → Hosts.",
            ),
            Role::Button,
        )
        .tier(Tier::Neutral)
        .compact()
        .when(pending, |button| {
            button.busy(true).accessible_name(waiting("Always update"))
        })
        .on_click(move |_, window, cx| {
            let mark = Mark::take(cx);
            status::always_update(&updates, mark, window, cx);
        })
    });
    let dismiss = optional.then(|| {
        let list = list.clone();
        IconButton::new(
            "host-attention-dismiss",
            Icon::Close,
            "Dismiss optional host updates",
            // The old words stay true: a helm on another build is a restarted helm, a new UI session, which forgets
            // the dismissal (the module docs' NOTE).
            HoverHelp::tooltip(
                "Dismiss optional updates until another host has an update or the helm build changes",
            ),
            Role::Button,
        )
        .tier(Tier::Neutral)
        .compact()
        .on_click(move |_, window, cx| {
            let mark = Mark::take(cx);
            let _ = list.update(cx, |list, cx| list.dismiss_updates(mark, window, cx));
        })
    });
    div()
        .flex_none()
        .flex()
        .flex_row()
        .flex_wrap()
        .items_center()
        .gap(px(HEADER_GAP))
        .p(px(HEADER_PAD))
        .child(
            div()
                .id("host-attention-heading")
                .role(Role::Label)
                .aria_label(SharedString::from(heading.clone()))
                .flex_1()
                .min_w(px(120.))
                .font_weight(FontWeight::SEMIBOLD)
                .child(heading),
        )
        .children(update_all)
        .children(always_update)
        .children(dismiss)
        .into_any_element()
}

/// Always update's failure under the header (`.host-attention-save-error`), an alert in the danger ink with its Try
/// again, which saves only the preference.
fn failure(palette: &Palette) -> AnyElement {
    div()
        .id("host-attention-save-error")
        .debug_selector(|| "host-attention-save-error".into())
        .role(Role::Alert)
        .aria_label(status::AUTO_UPDATE_FAILED)
        .flex_none()
        .flex()
        .flex_row()
        .flex_wrap()
        .items_center()
        .gap(px(HEADER_GAP))
        .px(px(HEADER_PAD))
        .pb(px(HEADER_PAD))
        .text_color(theme::color(palette.danger))
        .child(status::AUTO_UPDATE_FAILED)
        .child(
            Button::new(
                "host-attention-try-again",
                "Try again",
                HoverHelp::tooltip(
                    "Try saving automatic updates again without requesting these host updates again",
                ),
                Role::Button,
            )
            .tier(Tier::Neutral)
            .compact()
            .on_click(|_, _, cx| status::retry_auto_update(cx)),
        )
        .into_any_element()
}

/// One host's row (`.host-attention-row`): its open control (the mark, the short name, the state and, on a required
/// update, its consequence; on a working row the bar too), then Details on a problem row or Update on an update row.
/// The open control is the row's keyboard stop and spans the row up to its button, so a press anywhere outside the
/// button opens Hosts at the host.
fn row_element(
    row: &Row,
    launch_pending: bool,
    list: &WeakEntity<SessionList>,
    palette: &Palette,
) -> AnyElement {
    let id = row.host.id;
    let current = row.steps.as_ref().and_then(|steps| steps.current.as_ref());
    // The row's name and plain hover help, the old tooltip's words: "<identity>: <state> · <step>; open Hosts for
    // details". The step is host text, escaped in both.
    let mut sentence = HostSentence::new(SharedString::from(format!("host-attention-help-{id}")))
        .text(format!("{}: {}", identity(&row.host), row.words));
    if let Some(step) = current {
        sentence = sentence.text(" · ").host(step);
    }
    let sentence = sentence.text("; open Hosts for details");
    let label = sentence.accessible_name();
    // A working row with a run's steps shows the progress card, made only while it shows; before the steps arrive it
    // shares the plain hover help.
    let help = match (&row.kind, &row.steps) {
        (Kind::Working, Some(_)) => {
            HoverHelp::card_with(px(CARD_WIDTH), move |_, cx| card(id, cx), sentence).beside()
        }
        _ => HoverHelp::sentence(sentence),
    };
    let open = {
        let list = list.clone();
        move |_: &gpui::ClickEvent, window: &mut Window, cx: &mut App| {
            let mark = Mark::take(cx);
            let _ = list.update(cx, |list, cx| {
                list.show_attention_host(id, mark, window, cx)
            });
        }
    };
    let mut control = ListRow::new(
        format!("host-attention-open-{id}"),
        label,
        help,
        Role::Button,
    )
    .line(identity_line(row, current, palette))
    .on_click(open);
    if row.kind == Kind::Working {
        control = control.line(div().pr(px(ROW_PAD_RIGHT)).child(bar(
            id,
            row.steps.as_ref(),
            palette,
        )));
    }
    let action = match row.kind {
        Kind::Problem => {
            let list = list.clone();
            Some(
                Button::new(
                    format!("host-attention-details-{id}"),
                    "Details",
                    HoverHelp::tooltip("Details: open host management at this host"),
                    Role::Button,
                )
                .tier(Tier::Neutral)
                .compact()
                .on_click(move |_, window, cx| {
                    let mark = Mark::take(cx);
                    let _ = list.update(cx, |list, cx| {
                        list.show_attention_host(id, mark, window, cx)
                    });
                }),
            )
        }
        Kind::Update { .. } => {
            let help = if launch_pending {
                HoverHelp::tooltip(LAUNCH_PENDING)
            } else {
                update::hover(&row.host.state).map_or_else(
                    || HoverHelp::tooltip("Update: install this helm's version on this host"),
                    HoverHelp::sentence,
                )
            };
            Some(
                Button::new(
                    format!("host-attention-update-{id}"),
                    "Update",
                    help,
                    Role::Button,
                )
                .tier(Tier::Neutral)
                .compact()
                .when(launch_pending, |button| {
                    button.busy(true).accessible_name(waiting("Update"))
                })
                .on_click(move |_, window, cx| {
                    let mark = Mark::take(cx);
                    update::start(id, mark, window, cx);
                }),
            )
        }
        Kind::Working => None,
    };
    div()
        .id(SharedString::from(format!("host-attention-row-{id}")))
        .debug_selector(move || format!("host-attention-row-{id}"))
        .flex()
        .flex_row()
        .items_center()
        .border_t_1()
        .border_color(theme::color(palette.border_nested))
        .child(div().flex_1().min_w_0().child(control))
        .children(action.map(|action| div().flex_none().pr(px(ROW_PAD_RIGHT)).child(action)))
        .into_any_element()
}

/// The open control's line: the host's mark beside its short name, its state (with the current step after a working
/// row's verb, cut short to fit) and a required update's consequence, which may wrap.
fn identity_line(row: &Row, current: Option<&HostText>, palette: &Palette) -> AnyElement {
    let state_ink = match row.kind {
        Kind::Problem => palette.warn_bright,
        Kind::Working | Kind::Update { .. } => palette.fg_1,
    };
    let state = div()
        .flex()
        .flex_row()
        .min_w_0()
        .overflow_hidden()
        .whitespace_nowrap()
        .text_color(theme::color(state_ink));
    let state = match current {
        // The verb stays whole and the step gives way.
        Some(step) => state
            .child(div().flex_none().child(format!("{} · ", row.words)))
            .child(host_text::element(step).min_w_0().truncate()),
        None => state.child(div().min_w_0().truncate().child(row.words.clone())),
    };
    let required = (row.kind == Kind::Update { required: true }).then(|| {
        div()
            .text_color(theme::color(palette.warn_bright))
            .child(REQUIRED)
    });
    div()
        .w_full()
        .min_w_0()
        .pr(px(ROW_PAD_RIGHT))
        .flex()
        .flex_row()
        .items_start()
        .gap(px(MARK_GAP))
        .child(div().flex_none().mt(px(MARK_DROP)).child(host_mark(
            row.host.icon,
            row.host.color,
            px(MARK),
            palette,
        )))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .child(
                    host_text::element(&short_name(&row.host))
                        .font_weight(FontWeight::SEMIBOLD)
                        .truncate(),
                )
                .child(state)
                .children(required),
        )
        .into_any_element()
}

/// A working row's bar (`.host-attention-progress`): filled as `steps` complete when the run has a step count, empty
/// and still otherwise (only requests out, or no steps sent yet), always the same height so the row never moves.
fn bar(id: i64, steps: Option<&Steps>, palette: &Palette) -> AnyElement {
    let fill = steps
        .filter(|steps| steps.total > 0)
        .map(|steps| steps.done.min(steps.total) as f32 / steps.total as f32);
    let name = match steps.filter(|steps| steps.total > 0) {
        Some(steps) => format!(
            "Host operation progress: {} of {} steps",
            steps.done.min(steps.total),
            steps.total
        ),
        None => "Host operation in progress".to_owned(),
    };
    div()
        .id(SharedString::from(format!("host-attention-progress-{id}")))
        .debug_selector(move || format!("host-attention-progress-{id}"))
        .role(Role::ProgressIndicator)
        .aria_label(SharedString::from(name))
        .w_full()
        .h(px(BAR_HEIGHT))
        .rounded(metrics::RADIUS_SM)
        .bg(theme::color(palette.well))
        .when_some(fill, |bar, fill| {
            bar.child(
                div()
                    .debug_selector(move || format!("host-attention-progress-fill-{id}"))
                    .h_full()
                    .w(relative(fill))
                    .rounded(metrics::RADIUS_SM)
                    .bg(theme::color(palette.accent)),
            )
        })
        .into_any_element()
}

/// The accessible name of an update action held back while a session launch is in flight, `label` with why: a busy
/// button cannot report its state to assistive technology, so its name says it cannot act (`components::Button::busy`),
/// in the session header's "Restart unavailable" form, followed by the launcher triggers' reason
/// ([`LAUNCH_PENDING`]), "Update all unavailable: wait for the pending launch to finish".
fn waiting(label: &str) -> String {
    let mut reason = LAUNCH_PENDING.to_owned();
    if let Some(first) = reason.get_mut(..1) {
        first.make_ascii_lowercase();
    }
    format!("{label} unavailable: {reason}")
}

/// The progress card of host `id`'s run, made when the working row's hover shows; an empty view if the run ended just
/// before, which the hover drops with the row's next help.
fn card(id: i64, cx: &mut App) -> AnyView {
    progress_card(id, cx).map_or_else(|| cx.new(|_| gpui::EmptyView).into(), Into::into)
}
