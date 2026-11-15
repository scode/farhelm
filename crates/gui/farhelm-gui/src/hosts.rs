//! Host management: the Hosts panel the sidebar shows in place of its session list while its Hosts control is pressed,
//! and the per-host rules behind it.
//!
//! SPEC.md, "Session list" (the Hosts control, host rows, the update button and Too new) and "Topology", ported from the
//! old app's `hosts.rs` (`HostsPanel`, `HostRow`, the phase vocabulary) and `provisioning.rs`
//! (`ProvisioningMenuState::offered`) and the stylesheet's `.hosts-heading` and `.host-row` rules at reference commit
//! `24bc0580`.
//!
//! ## Entry points
//!
//! [`HostsPanel`] is the view. The sidebar makes one with `cx.new(HostsPanel::new)` and keeps it for the window's life,
//! drawing it as a child in place of the session list's column while Hosts is pressed and not drawing it otherwise.
//! Keeping the one instance across the swap is what keeps the Details choice (SPEC.md: opening or closing Hosts "keeps
//! the current ... details choice"); nothing in the panel needs to run while it is hidden, since everything it shows is
//! read from the store when it draws. It fills the box it is given and scrolls its rows inside it, so the box needs a
//! definite height.
//!
//! [`open_menu`] opens a host's actions menu at an anchor ([`menu`]): the panel's row toggle calls it, and the host
//! rail's avatar menu will. [`open_settings`] opens a host's settings dialog ([`settings`]), which the menu's Settings
//! calls.
//!
//! The Add host dialog is [`add::open`], which the host rail's Add host and the heading's add button (`+`) call, each
//! naming itself as where closing the dialog returns focus. A new host without Farhelm goes on to the setup
//! confirmation and the setup's submission ([`setup`]); a setup the helm refused after the user moved on from it shows
//! at the top of the panel, since it has no row. Add host, a setup's submission, a row's Re-run and Remove refuse to
//! start while a session launch is in flight ([`launch_in_flight`]), as the old app refused a host change during a
//! launch.
//!
//! SSH help ("SSH needs 2FA?"), the commands that start a host's two shared SSH connections by hand, is
//! [`ssh_help::open`], the entry point for the ? menu's "SSH needs 2FA?" (drawn once the area that owns the ? menu
//! lands); Add host opens it for the destination typed there.
//!
//! The menu's Remove asks the removal confirmation, or with "Remove hosts without asking" on goes ahead at once
//! ([`remove`]). A refused removal shows under its row, like any refused work on a host; a Remove that was not started
//! at all (the host busy, a session launch in flight) says so at the top of the panel.
//!
//! How a host is named and marked, here and in every other area that shows a host, is [`identity`].
//!
//! ## What it shows
//!
//! A heading band with the host count ("Hosts 3", read out as "3 hosts"), the Details switch and the add button, then
//! one row per host in the registry's order, session-owned hosts included (SPEC.md: they "participate in ordinary host
//! management"). A row is the host's identity mark, its name (the alias, else the helm's display name, "Local (this
//! machine)" for the local host under the registry's own name; the destination behind an alias shows in the details),
//! its phase dot and words ([`identity::phase`]), and the ⋯ toggle that opens its actions menu. Under it, while the row
//! is open, the details: the destination behind an alias, what the helm knows about the phase (for a connected host its
//! version, identity and session count) and the remedy where waiting will not help. The last refusal of work on the
//! host shows under the row whether it is open or not.
//!
//! A row is open while the Details switch is on, or while the row opens itself ([`disclosed`]): a setup is running on
//! it that the user has not folded, its last provisioning run failed, the last work on it was refused, or it is the
//! helm's own machine with no supervisor running. The switch is the user's alone: nothing else turns it on, and it is
//! not saved anywhere (SPEC.md: "its unpersisted details switch"). The old app turned the switch on for a failed local
//! setup; the port opens that row instead.
//!
//! A host with a setup running, or a rerun of its setup out, shows the run's inline status on a line of its own under
//! its name in place of the phase words, and an open row shows the run's steps under its details ([`progress`], which
//! also keeps the one fact the panel holds about a host: when it first saw the run, and whether the user folded it).
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
//! NOTE: Each arrives with the piece of the port that gives it something to do: Update all in the heading, the inline
//! `↑ Update` button, and Update's progress. Until Update all arrives, the free space it took in the old heading goes
//! to the two automatic margins, so Details sits further from the count than in the old app.

pub mod add;
pub(crate) mod identity;
mod menu;
pub(crate) mod offers;
pub(crate) mod progress;
pub(crate) mod remove;
mod settings;
pub(crate) mod setup;
pub mod ssh_help;

pub use menu::open_menu;
pub use settings::open_settings;

#[cfg(test)]
pub(crate) mod tests;

use std::collections::HashMap;

use farhelm_proto::provisioning::ProvisioningView;
use farhelm_proto::ui_channel::{Availability, Collection, HostStateView, HostView};
use gpui::{
    AnyElement, App, Context, Div, FocusHandle, FontWeight, InteractiveElement, IntoElement,
    ParentElement, Render, Role, SharedString, StatefulInteractiveElement, Styled, Subscription,
    WeakEntity, Window, div, px,
};

use crate::assets::Icon;
use crate::components::{Checkbox, HoverHelp, IconButton, StatusMark, Tier, pulse_opacity};
use crate::focus::Mark;
use crate::host_text::{self, HostSentence, HostText};
use crate::layers::{self, Anchor, LayerId};
use crate::operations::{Failure, OpKey, Work};
use crate::render_count;
use crate::session::{self, PerSession};
use crate::theme::{self, Palette, Token, metrics};
use crate::ticker;
use identity::phase::{self, Kind, Tone};
use identity::{display_name, host_mark};
use offers::{HostKind, Locality};

/// `.hosts-heading`'s padding (vertical then horizontal) and the gap between its parts.
const HEADING_PAD: (f32, f32) = (4., 10.);
const HEADING_GAP: f32 = 8.;

/// `.hosts-heading .add-host-button`: the add button's side, which with the padding sets the heading's 32-pixel height.
const ADD_SIZE: f32 = 24.;

/// What a host registry change refuses with while a session launch is in flight ([`launch_in_flight`]), after the name
/// of the action that was not started ("Add host: ", or "Set up <host>: ", "Re-run <host>: ", "Remove <host>: " on the
/// Hosts panel's line, "Destination: " in the host settings): the old app's form for an action refused while another
/// runs ("<action>: not started, another operation is in progress; try again", `list/lifecycle.rs` at the reference
/// commit), naming the launch.
pub(crate) const LAUNCH_IN_FLIGHT: &str =
    "not started, a session launch is in progress; try again when it finishes";

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

/// The gap between a row's name and the inline progress on the line under it (`.host-row-main`'s `row-gap` while it
/// shows progress).
const PROGRESS_GAP: f32 = 2.;

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
    /// The host whose menu a row toggle last opened, and the menu's layer: what draws that toggle pressed and makes a
    /// second press close the menu. Checked with the layer manager whenever it is read, so a menu closed some other
    /// way (Escape, a press outside, a chosen command) needs no bookkeeping here.
    menu: Option<(i64, LayerId)>,
    /// Each drawn row's toggle, by host id, as last drawn: its anchor and its focus handle, which a press of the toggle
    /// opens the menu at ([`HostsPanel::toggle_menu`]). A row not drawn last frame has none.
    toggles: HashMap<i64, (Anchor, FocusHandle)>,
    /// The heading's add button's focus handle, kept for the panel's life so the Add host dialog it opens can return
    /// focus to it by name when it closes.
    add_focus: FocusHandle,
    /// Redraws the panel when its top line changes (a setup refused after the user moved on, a refused "don't ask
    /// again"): that line lives in a global of the setup module, which notifies no view by itself.
    _lines: Subscription,
    /// The same for the removal module's lines (a Remove that was not started, a refused "don't ask again").
    _removal_lines: Subscription,
    /// What the panel keeps about the hosts' runs ([`Runs`]), for the current UI session only: a reset starts it over.
    runs: PerSession<Runs>,
    /// Keeps `runs` up to date with every change of the current store, whether or not the panel is drawn, so a run's
    /// clock starts when it is first pushed even while Hosts is closed, and starts it over on every change of UI
    /// session, a reset's included.
    _following: session::ObservingCurrent,
}

/// What the panel keeps about the hosts' runs for one UI session, all of it dropped with that session: a reset starts
/// the fold and the clock over, as the old app's reload did, and lets go of the pulse and the clock's deadline, which
/// the next draw takes again if it needs them.
#[derive(Default)]
struct Runs {
    /// Each host's run as the panel saw it ([`progress::Seen`]), by host id.
    seen: HashMap<i64, progress::Seen>,
    /// The shared pulse's ticks, held exactly while the panel is drawn in an active window and a row shows a setup in
    /// progress.
    pulse: Option<ticker::Pulsing>,
    /// The deadline that redraws the panel when a shown run's elapsed time reaches its next second (`progress`'s module
    /// docs say why it is a deadline); held only while the panel is drawn and a run shows, and taken when it fires.
    tick: Option<ticker::Deadline>,
}

impl HostsPanel {
    /// A panel with Details off. It reads everything else from the store while it draws, so it can be made before the
    /// connection or the first snapshot and needs nothing from them.
    pub fn new(cx: &mut Context<Self>) -> Self {
        HostsPanel {
            details: false,
            menu: None,
            toggles: HashMap::new(),
            add_focus: cx.focus_handle().tab_stop(true),
            _lines: setup::observe_panel_lines(cx),
            runs: PerSession::default(),
            _following: session::observe_current(cx, |panel: &mut Self, cx| panel.follow_runs(cx)),
            _removal_lines: remove::observe_panel_lines(cx),
        }
    }

    /// Record what the current store's host progress says about each host's run ([`progress::follow`]), now. Reaching
    /// the runs through [`PerSession::current`] here, on every change of the connection, is also what drops them, the
    /// pulse and the deadline included, as soon as a reset replaces the UI session, whether or not the panel is drawn.
    fn follow_runs(&mut self, cx: &mut Context<Self>) {
        let now = ticker::wall_now(cx);
        let runs = &mut self.runs.current(cx).seen;
        let Some(store) = session::current_store(cx) else {
            return;
        };
        progress::follow(
            runs,
            store.read(cx).collections().ordered::<ProvisioningView>(),
            now,
        );
    }

    /// The sidebar stopped drawing the panel (Sessions shows instead): drop the pulse and the clock's deadline. Only a
    /// drawn panel needs them, and the panel cannot tell it is not drawn, since only drawing runs its code, so without
    /// this a pulse taken while Hosts showed would keep the ticker's loop waking ten times a second for a panel nobody
    /// sees (the ticker's rule: no view holds a pulse while it draws no pulsing mark). The sidebar calls it where it
    /// hides the panel; a reset needs no call ([`HostsPanel::follow_runs`]). The next draw takes what it needs again.
    pub fn hidden(&mut self, cx: &App) {
        let runs = self.runs.current(cx);
        runs.pulse = None;
        runs.tick = None;
    }

    /// Fold `host`'s run `run_id`, as its Collapse does: the row closes unless something else holds it open, and the
    /// inline status stays.
    fn fold(&mut self, host: i64, run_id: &str, cx: &mut Context<Self>) {
        progress::fold(&mut self.runs.current(cx).seen, host, run_id);
        cx.notify();
    }

    /// Turn Details on or off, as the switch does.
    fn set_details(&mut self, on: bool, cx: &mut Context<Self>) {
        self.details = on;
        cx.notify();
    }

    /// `host`'s row toggle was pressed: close its menu if it is open, and open it otherwise, beside the toggle with
    /// focus going back to it when it closes ([`open_menu`]). Does nothing for a row not drawn last frame. Another
    /// host's open menu has already closed by then, since the press was outside it. Visible to the crate for the
    /// screenshot goldens, which open a menu without a click.
    pub(crate) fn toggle_menu(&mut self, host: i64, window: &mut Window, cx: &mut Context<Self>) {
        let Some((anchor, opener)) = self.toggles.get(&host).cloned() else {
            return;
        };
        if let Some((open, layer)) = self.menu.take()
            && open == host
            && layers::is_open(layer, window, cx)
        {
            layers::close(layer, window, cx);
            cx.notify();
            return;
        }
        let mark = Mark::take(cx);
        self.menu = open_menu(host, anchor, &opener, &mark, window, cx).map(|layer| (host, layer));
        cx.notify();
    }

    /// Whether `host`'s row toggle was drawn last frame, so a golden scene presses it only once that press can open
    /// the menu.
    #[cfg(test)]
    pub(crate) fn toggle_drawn(&self, host: i64) -> bool {
        self.toggles.contains_key(&host)
    }

    /// Turn Details on or off from a test or a golden scene, as a toggle of the switch would.
    #[cfg(test)]
    pub(crate) fn show_details(&mut self, on: bool, cx: &mut Context<Self>) {
        self.set_details(on, cx);
    }
}

/// Whether `host`'s row is open: the Details switch is on, a setup is running on the host that the user has not folded
/// (`progress`, and what the panel keeps about it, `seen`), the host's last provisioning run failed, the last work on
/// the host was refused (`refused`), or the host is the helm's own machine with no supervisor running. All but the
/// switch open the row by themselves: a running setup once, so its steps show as they go (SPEC.md: "the running row
/// expands once"), a failure always, so it shows its evidence and remedies whatever was folded, and the local host
/// without a supervisor so it shows what to do rather than reading as a phantom unreachable host (SPEC.md, "Topology").
/// Each closes it again once it no longer holds: success ends the first, a new run or new work on the host replaces
/// the next two, and a supervisor starting ends the last.
pub(crate) fn disclosed(
    details: bool,
    host: &HostView,
    progress: Option<&ProvisioningView>,
    seen: Option<&progress::Seen>,
    refused: bool,
) -> bool {
    details
        || progress::opens_row(progress, seen)
        || offers::failed_operation(progress).is_some()
        || refused
        || offers::needs_local_setup(host)
}

/// What one row is drawn from, borrowed from the store and the operation store for the frame being drawn, with what the
/// panel keeps about its run and the inline status worked out from them.
struct RowInput<'a> {
    host: &'a HostView,
    progress: Option<&'a ProvisioningView>,
    refusal: Option<&'a Failure>,
    seen: Option<&'a progress::Seen>,
    status: Option<progress::Status>,
}

impl Render for HostsPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        render_count::record("hosts-panel", cx);
        let palette = theme::palette(cx);
        let weak = cx.weak_entity();
        let mut toggles = self.toggles(window, cx);
        let operations = session::current_operations(cx);
        let now = ticker::wall_now(cx);
        // Copied out for the frame: the rows borrow the store, and `current` needs the panel mutably. A handful of
        // entries at most, one per host with a run.
        let seen = self.runs.current(cx).seen.clone();
        // The running pulse's opacity now, which every row's setup dot and running step share.
        let pulse = pulse_opacity(
            self.runs
                .current(cx)
                .pulse
                .as_ref()
                .map_or(0, |pulse| pulse.step(cx)),
            palette.status_pulse_min,
        );
        // Whether a row shows setup in progress (for the pulse), and when the soonest shown clock turns over.
        let mut pulsing = false;
        let mut next_second: Option<std::time::Duration> = None;
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
                        let op_key = OpKey::host(&key);
                        let progress = collections.get::<ProvisioningView>(&key);
                        let seen = seen.get(&host.id);
                        let in_flight: Option<Work> =
                            operations.and_then(|operations| operations.in_flight(&op_key));
                        let status = progress::status(progress, seen, in_flight, now);
                        pulsing |= status.is_some();
                        if let Some(progress::Status::Running { elapsed, .. }) = &status {
                            let due = progress::next_second(*elapsed);
                            next_second = Some(next_second.map_or(due, |soonest| soonest.min(due)));
                        }
                        let input = RowInput {
                            host,
                            progress,
                            refusal: operations.and_then(|operations| operations.failure(&op_key)),
                            seen,
                            status,
                        };
                        self.row(
                            &input,
                            &twin,
                            toggles.remove(&host.id),
                            &weak,
                            pulse,
                            palette,
                        )
                    })
                    .collect();
            }
        }

        self.keep_time(pulsing, next_second, window, cx);
        let heading = div()
            .flex()
            .flex_row()
            .flex_wrap()
            .items_center()
            .gap(px(HEADING_GAP))
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
            )
            // The old actions group's automatic margin (`.hosts-heading-actions`), beside the switch's, as the old
            // sheet gave both: the two share the heading's free space, and the button keeps the trailing edge. A
            // button takes no margin of its own, so this holder carries it.
            .child(div().ml_auto().child(add_button(&self.add_focus)));
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
        // A setup refused after the user moved on from it, and a refused "don't ask again": no row exists for a host
        // the helm did not add, and the question that asked has closed, so they show here (`setup::panel_lines`). A
        // Remove that was not started shows here too, for now (`remove::panel_lines`).
        let refused_setup = setup::panel_lines(cx).into_iter();
        let refused_removal = remove::panel_lines(cx).into_iter();
        let refused = refused_setup.chain(refused_removal).map(|line| {
            div()
                .py(px(4.))
                .px(px(10.))
                .text_size(metrics::TEXT_SM)
                .text_color(theme::color(palette.danger))
                .child(line)
        });
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
            .children(refused)
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
    /// Hold the pulse while the window is active and a row shows setup in progress (`pulsing`), and while a run's clock
    /// shows, a deadline that redraws the panel when the soonest one turns over (`next_second`). The ticker's rule for
    /// the pulse: never held for a window nobody is looking at, where it would keep the tick loop firing for nothing.
    fn keep_time(
        &mut self,
        pulsing: bool,
        next_second: Option<std::time::Duration>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let pulses = pulsing && ticker::is_active(window, cx);
        let runs = self.runs.current(cx);
        if pulses != runs.pulse.is_some() {
            runs.pulse = pulses.then(|| ticker::pulse(window, cx));
        }
        match next_second {
            None => runs.tick = None,
            Some(due) if runs.tick.is_none() => {
                let panel = cx.weak_entity();
                runs.tick = Some(ticker::after(due, cx, move |cx| {
                    let _ = panel.update(cx, |panel, cx| {
                        panel.runs.current(cx).tick = None;
                        cx.notify();
                    });
                }));
            }
            Some(_) => {}
        }
    }

    /// Every listed host's menu toggle ([`menu::toggle`]), by host id, the one whose menu is open drawn pressed, with
    /// each one's anchor and focus handle recorded for [`HostsPanel::toggle_menu`]. Made before the rows, which borrow
    /// the store, since a toggle's anchor and focus handle are element state.
    fn toggles(&mut self, window: &mut Window, cx: &mut Context<Self>) -> HashMap<i64, AnyElement> {
        let listed: Vec<(i64, HostText)> = session::current_store(cx)
            .map(|store| {
                let collections = store.read(cx).collections();
                collections
                    .ordered::<HostView>()
                    .into_iter()
                    .map(|host| (host.id, display_name(host)))
                    .collect()
            })
            .unwrap_or_default();
        let open = self
            .menu
            .filter(|(_, layer)| layers::is_open(*layer, window, cx))
            .map(|(host, _)| host);
        self.toggles.clear();
        listed
            .into_iter()
            .map(|(id, name)| {
                let parts = menu::toggle_parts(id, window, cx);
                let panel = cx.weak_entity();
                let toggle =
                    menu::toggle(id, &name, open == Some(id), &parts, move |window, cx| {
                        let _ = panel.update(cx, |panel, cx| panel.toggle_menu(id, window, cx));
                    });
                self.toggles.insert(id, parts);
                (id, toggle)
            })
            .collect()
    }

    /// One host's row (`.host-row`): its header line ending in its menu `toggle` (with the inline status of a setup in
    /// progress under it), then its details while it is open, then the last refusal of work on it, which shows open or
    /// not, then a setup run's steps while it is open. `twin` names a host by id for a duplicate's details; `panel` is
    /// what a run's Collapse folds it in, and `pulse` the running pulse's opacity for this frame.
    fn row(
        &self,
        input: &RowInput,
        twin: &impl Fn(i64) -> Option<HostText>,
        toggle: Option<AnyElement>,
        panel: &WeakEntity<HostsPanel>,
        pulse: f32,
        palette: &Palette,
    ) -> AnyElement {
        let host = input.host;
        let id = host.id;
        let name = display_name(host);
        let open = disclosed(
            self.details,
            host,
            input.progress,
            input.seen,
            input.refusal.is_some(),
        );
        let line = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(ROW_GAP))
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
            );
        let main = div()
            .flex()
            .flex_col()
            .justify_center()
            .gap(px(PROGRESS_GAP))
            .min_h(px(ROW_HEIGHT))
            .py(px(ROW_PAD.0))
            .pl(px(ROW_PAD.1));
        // Progress takes a line of its own under the name, lined up with it, so a long step cannot take the name's
        // place; the toggle stays on the name's line (`.host-row-main:has(.host-status.updating)`).
        let main = match &input.status {
            Some(status) => main.child(line.children(toggle)).child(
                div()
                    .pl(px(MARK + ROW_GAP))
                    .pr(px(DETAIL_PAD_X))
                    .min_w_0()
                    .child(progress::status_mark(id, status, pulse, palette)),
            ),
            None => main.child(line.child(phase_status(host, palette)).children(toggle)),
        };
        let lines: Vec<AnyElement> = if open {
            phase::detail_lines(host, twin)
                .into_iter()
                .map(|(kind, line)| detail_line(kind, line, palette))
                .collect()
        } else {
            Vec::new()
        };
        // The run's steps under an open row, with Collapse while the row is open for the run alone and the user may fold
        // it (with Details on, the switch holds every row open, so there is nothing to fold).
        let run = progress::shown(input.progress).filter(|_| open).map(|run| {
            let collapse = (!self.details && progress::opens_row(Some(run), input.seen))
                .then(|| run.run_id.clone())
                .flatten()
                .map(|run_id| {
                    let panel = panel.clone();
                    progress::collapse(id, move |_, cx| {
                        let _ = panel.update(cx, |panel, cx| panel.fold(id, &run_id, cx));
                    })
                });
            progress::run_section(id, run, collapse, pulse, palette)
        });
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
        // shows open or not, then the run's steps. Nothing at all when there is none of them, so a closed row is its
        // header alone.
        let under = (!lines.is_empty() || refusal.is_some() || run.is_some()).then(|| {
            div()
                .pb(px(DETAILS_PAD_BOTTOM))
                .children(lines)
                .children(refusal)
                .children(run)
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

/// The heading's add button (`.hosts-heading .add-host-button`): the old Add glyph, a plus, in a 24-pixel neutral
/// square, named "Add host". It opens the Add host dialog with `focus`, its kept handle, as the opener, so closing the
/// dialog returns focus to it, as the host rail's Add host does for its own. Never disabled, like the rail's: the old
/// button was, while the old page's one token for launches and host registry changes was held, but here the dialog's
/// Add refuses during a launch instead ([`launch_in_flight`]).
fn add_button(focus: &FocusHandle) -> AnyElement {
    let opener = focus.clone();
    IconButton::new(
        "hosts-add",
        Icon::Add,
        "Add host",
        HoverHelp::tooltip(add::ADD_HOST_HELP),
        Role::Button,
    )
    .tier(Tier::Neutral)
    .square(px(ADD_SIZE))
    .focus(focus)
    .on_click(move |_, window, cx| {
        // Taken in the handler, after the coordinator counted this press or key, so the dialog opens for this press and
        // not for one the user has since moved on from.
        let mark = Mark::take(cx);
        add::open(&opener, &mark, window, cx);
    })
    .into_any_element()
}

/// Whether a session launch is in flight anywhere in the operation store: what a host registry change from this module
/// (Add host's probe, a setup's submission, a row's Re-run, a host's removal) refuses to start during. The old app
/// refused a host change during a launch and the reverse, through one page-wide token; the migration's orchestrator
/// decided on 2026-11-14 that each side refuses its own action instead, with a reason, while the other's work is in
/// flight in the operation store. `false` in an inert window, which has no store and starts nothing anyway.
pub(crate) fn launch_in_flight(cx: &App) -> bool {
    session::current_operations(cx)
        .is_some_and(|operations| operations.read(cx).any_in_flight(creates_a_session))
}

/// Whether `work` creates a session on a host, which is what "a launch in flight" means for a host registry change:
/// the launcher's New and Clone, and a session's Restart and Replace, each of which starts a session on its host
/// (decided by the migration's orchestrator on 2026-11-14). Exhaustive with no catch-all, so a new kind of work must
/// decide whether it counts.
fn creates_a_session(work: Work) -> bool {
    match work {
        Work::Launch | Work::Restart | Work::Replace => true,
        Work::Rename
        | Work::Delete(_)
        | Work::Save
        | Work::Stop
        | Work::Seen
        | Work::Probe
        | Work::Setup
        | Work::TunnelOpen
        | Work::TunnelClose
        | Work::RemoveHost => false,
    }
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
            .content(phase_dot_and_words(&host.state, palette)),
        )
        .into_any_element()
}

/// A phase's dot and words in its tone's ink, on one line, the words cut short when they do not fit: what a row's
/// status and the host settings' header both show (the old `.host-status` span, which both drew).
fn phase_dot_and_words(state: &HostStateView, palette: &Palette) -> Div {
    let ink = tone_ink(phase::tone(state), palette);
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(STATUS_GAP))
        .whitespace_nowrap()
        .text_color(theme::color(ink))
        .child(
            div()
                .flex_none()
                .size(metrics::STATUS_DOT)
                .rounded_full()
                .bg(theme::color(ink)),
        )
        .child(div().min_w_0().truncate().child(phase::words(state)))
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
/// ordinary ink and the rest in the quiet one. The sentence is the line's one labelled node for assistive technology;
/// this wrapper adds only the line's spacing and ink, which the sentence's parts inherit.
///
/// NOTE: The parts wrap as whole pieces, as every sentence mixing the app's words with host text does
/// ([`HostSentence`]): a long value starts on a line of its own and wraps inside itself, where the old page broke it
/// mid-run after the words before it.
fn detail_line(kind: Kind, line: HostSentence, palette: &Palette) -> AnyElement {
    let ink = match kind {
        Kind::Evidence => palette.fg_1,
        Kind::Remedy => palette.fg_0,
    };
    div()
        .pt(px(DETAIL_PAD_TOP))
        .px(px(DETAIL_PAD_X))
        .text_size(metrics::TEXT_SM)
        .line_height(px(DETAIL_LINE))
        .text_color(theme::color(ink))
        .child(line)
        .into_any_element()
}

/// A host dialog's error line (`.create-session-error`): `line`, which names the helm's or a host's words through the
/// sentence's host parts, at 12 pixels in the danger ink, padded as the old line was. Add host's line and SSH help's
/// refusals and unavailable connections share it, so the two dialogs' errors cannot drift apart.
fn problem_line(line: HostSentence, palette: &Palette) -> AnyElement {
    div()
        .pt(px(2.))
        .px(px(14.))
        .pb(px(8.))
        .text_size(metrics::TEXT_SM)
        .text_color(theme::color(palette.danger))
        .child(line)
        .into_any_element()
}
