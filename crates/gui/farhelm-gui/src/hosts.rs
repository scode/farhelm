//! Host management: the Hosts panel the sidebar shows in place of its session list while its Hosts control is pressed,
//! and the per-host rules behind it.
//!
//! SPEC.md, "Session list" (the Hosts control, host rows, the update button and Too new) and "Topology", ported from
//! the old app's `hosts.rs` (`HostsPanel`, `HostRow`, the phase vocabulary) and `provisioning.rs`
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
//! [`open_menu`] opens a host's actions menu at an anchor ([`menu`]): the panel's row toggle calls it, and so does the
//! host rail's avatar on a secondary click or Shift+F10. [`open_settings`] opens a host's settings dialog
//! ([`settings`]), which the menu's Settings calls.
//!
//! The Add host dialog is [`add::open`], which the host rail's Add host and the heading's add button (`+`) call, each
//! naming itself as where closing the dialog returns focus. A new host without Farhelm goes on to the setup
//! confirmation and the setup's submission ([`setup`]); a setup the helm refused after the user moved on from it shows
//! at the top of the panel, since it has no row. Add host, a setup's submission, a row's Re-run, Remove, Update, Update
//! all, Uninstall, Retry and Adopt refuse to start while a session launch is in flight ([`launch_in_flight`]), as the
//! old app refused a host change during a launch.
//!
//! SSH help ("SSH needs 2FA?"), the commands that start a host's two shared SSH connections by hand, is
//! [`ssh_help::open`], the entry point for the ? menu's "SSH needs 2FA?". Add host opens it for the destination
//! typed there.
//!
//! The menu's Remove asks the removal confirmation, or with "Remove hosts without asking" on goes ahead at once
//! ([`remove`]). A removal asked through the confirmation reports a refusal inside it, which stays up through the
//! request, and so does one that was not started at all (the host busy, a session launch in flight). Only a Remove that
//! asked nothing reports outside a question: a refusal under its row, like any refused work on a host, and a Remove
//! not started at the top of the panel.
//!
//! The menu's Uninstall removes Farhelm from an SSH host ([`uninstall`]): it asks the helm for a plan, shows it for
//! confirmation, and shows the run in the host's row like a setup's. Once the run has forgotten the host, a one-line
//! removal notice at the top of the panel says where the host's data stays.
//!
//! The menu's Retry tries a host's connection again now, and its Adopt accepts the identity a mismatched host reports
//! ([`remedies`]); the duplicate entry's remedy ends in Retry, and fixing the destination instead is the host settings
//! dialog's.
//!
//! A remote host's Update ([`update`]) is sent the same way from the menu's Update and its rerun of a failed one, from
//! a row's inline `↑ Update` and, for every host whose Update is available, from the heading's Update all. A refused
//! Update plan opens the host refusal dialog ([`refusal`]) if the user has not moved on since the press. The attention
//! list's Update rows call [`update::start`] and [`update::start_all`] too.
//!
//! What the host rail and the attention list show about each host is [`status`], which draws nothing: the sidebar
//! reads [`status::host_status`] (the update badge, the work going on and the setup or Update warning) and
//! [`status::auto_updates_on`] and [`status::auto_update_failed`], redraws on [`status::observe_changes`], and calls
//! [`update::start`] for one host's Update, [`update::start_all`] for Update all over the hosts it shows,
//! [`status::always_update`] and [`status::retry_auto_update`]. A warning lasts until Hosts opens, so the sidebar calls
//! [`HostsPanel::shown`] wherever it shows the panel, as it calls [`HostsPanel::hidden`] wherever it hides it. The
//! attention list's Details shows the panel and then asks it to bring that host's row into view with its menu toggle
//! focused ([`HostsPanel::reveal`]).
//!
//! [`progress_card`] makes the progress card of a host's running setup, Update or uninstall ([`progress::card`]): the
//! hover card SPEC.md gives the Hosts row's inline status and the attention list's working rows alike. Both offer it as
//! lazy card help beside the sidebar: the row's running status ([`progress::status_mark`]) and the attention list's
//! working rows.
//!
//! How a host is named and marked, here and in every other area that shows a host, is [`identity`].
//!
//! ## What it shows
//!
//! A heading band with the host count ("Hosts 3", read out as "3 hosts"), the Details switch, Update all (unavailable
//! while no host's Update is) and the add button, then one row per host in the registry's order, session-owned hosts
//! included (SPEC.md: they "participate in ordinary host management"). A row is the host's identity mark, its name (the
//! alias, else the helm's display name, "Local (this machine)" for the local host under the registry's own name; the
//! destination behind an alias shows in the details), a suspended or stopped sandbox's or container's inline Resume
//! ([`lifecycle`]), its phase dot and words ([`identity::phase`]) with the inline `↑ Update` after them for a host on
//! an older build whose Update is available, and the ⋯ toggle that opens its actions menu. Under it, while the row is
//! open, the details: the destination behind an alias, what the helm knows about the phase (for a connected host its
//! version, identity and session count) and the remedy where waiting will not help. The last refusal of work on the
//! host shows under the row whether it is open or not.
//!
//! A row is open while the Details switch is on, or while the row opens itself ([`disclosed`]): a setup is running on
//! it that the user has not folded, its last provisioning run failed, the last work on it was refused, or it is the
//! helm's own machine with no supervisor running. The switch is the user's alone: nothing else turns it on, and it is
//! not saved anywhere (SPEC.md: "its unpersisted details switch"). The old app turned the switch on for a failed local
//! setup; the port opens that row instead.
//!
//! A host with a setup, an Update or an uninstall running, or its requests out, shows the run's inline status on a line
//! of its own under its name in place of the phase words and the inline Update, and an open row shows the run's steps
//! under its details ([`progress`], which also keeps the one fact the UI holds about a host's run: when it first saw the
//! run, and whether the user folded it). A running setup or uninstall opens its row; a running Update leaves it
//! folded, and its failure opens it like any failure. A Suspend's or Resume's request, and the provider's change after
//! it, also put the status on a line of its own, "Suspending…" or "Resuming…" with how long ago Farhelm asked, while an
//! inline Resume stays on the name's line.
//!
//! While any row shows a run's elapsed time (a setup, an Update or an uninstall) or how long ago Farhelm asked a
//! provider to suspend or resume a machine ([`phase::asked_ago`]), the panel holds the ticker's shared one-second clock
//! ([`ticker::every_second`]), which redraws it once a second in an inactive window and under reduced motion too, so
//! the count moves on with nothing pushed; while its window is active and a row shows a run in progress or a machine
//! changing, it also holds the shared pulse, which is the changing row's spinner (SPEC.md, "Topology"). It lets go of
//! both the moment the sidebar hides it ([`HostsPanel::hidden`]) and when a reset replaces the UI session, so a panel
//! nobody sees keeps no ticker loop awake.
//!
//! A host list never read says "Loading hosts…"; a failed first read says so in red; a failed refresh keeps the rows it
//! had and says above them that they are the last state read. The panel never invents a host.
//!
//! The helm's own machine with no supervisor running reads "Unreachable, retrying" and opens its own row to show the
//! helm's dial failure as evidence and to tell the user to quit Farhelm and open it again, since the desktop app starts
//! that machine's supervisor when it opens ([`identity::phase::remedy`]), so it never reads as a phantom unreachable
//! host (SPEC.md, "Topology"). It offers no install ([`offers`]). The old app also probed the machine for the helm's
//! `farhelm helm setup` handoff; the migration's settings-and-hosts area decided on 2026-11-14 that the row shows the
//! remedy instead, and no probe is planned.

pub mod add;
pub(crate) mod identity;
mod lifecycle;
mod menu;
pub(crate) mod offers;
pub(crate) mod progress;
pub(crate) mod refusal;
pub(crate) mod remedies;
pub(crate) mod remove;
mod settings;
pub(crate) mod setup;
pub(crate) mod setup_sign_in;
pub mod ssh_help;
pub mod status;
pub(crate) mod uninstall;
pub(crate) mod update;

pub use menu::open_menu;
pub use progress::card::{ProgressCard, progress_card};
pub use settings::open_settings;
// The launcher's YOLO question sends exactly what the host settings' "Start YOLO sessions without asking" switch
// sends.
pub(crate) use settings::start_yolo_without_asking_command;

#[cfg(test)]
mod environment_tests;
#[cfg(test)]
pub(crate) mod tests;

use std::collections::HashMap;

use farhelm_proto::provisioning::ProvisioningView;
use farhelm_proto::ui_channel::{Availability, Collection, HostStateView, HostView};
use gpui::{
    AnyElement, App, Context, Div, EntityId, FocusHandle, FontWeight, InteractiveElement,
    IntoElement, ParentElement, Render, Role, ScrollHandle, SharedString,
    StatefulInteractiveElement, Styled, Subscription, WeakEntity, Window, div,
    prelude::FluentBuilder as _, px,
};

use crate::assets::Icon;
use crate::components::{
    Button, Checkbox, HoverHelp, IconButton, MenuStart, StatusMark, Tier, pulse_opacity,
};
use crate::environment_ui::ProviderUi;
use crate::focus::{self, Mark};
use crate::host_text::{self, HostSentence, HostText};
use crate::layers::{self, Anchor, LayerId};
use crate::local_time::{self, Zone};
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

/// What a host action from this module refuses with while a session launch is in flight ([`launch_in_flight`]), after
/// the name of the action that was not started ("Add host: ", or "Set up <host>: ", "Re-run <host>: ",
/// "Remove <host>: ", "Update <host>: ", "Update all: ", "Uninstall <host>: ", "Retry <host>: ", "Adopt <host>: " on
/// the Hosts panel's line, "Destination: " in the host settings): the old app's form for an action refused while
/// another runs ("<action>: not started, another operation is in progress; try again", `list/lifecycle.rs` at the
/// reference commit), naming the launch.
pub(crate) const LAUNCH_IN_FLIGHT: &str =
    "not started, a session launch is in progress; try again when it finishes";

/// `.hosts-heading-actions`'s gap between Update all and the add button.
const ACTIONS_GAP: f32 = 4.;

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
    /// A host whose row the next frames are to bring into view with its toggle focused, and the mark of the press that
    /// asked ([`HostsPanel::reveal`]).
    reveal: Option<(i64, Mark)>,
    /// The rows' scroll state, kept across frames: the row list is a focus coordinator scroll region, which is how a
    /// focus move onto a toggle below the fold (Tab, or [`HostsPanel::reveal`]'s) scrolls it into view. Kept on the
    /// panel, which lives as long as the window, so the list keeps its scroll position when Hosts closes and opens
    /// again, as the old panel did: it stayed mounted, only hidden, while Sessions showed.
    scroll: ScrollHandle,
    /// The heading's add button's focus handle, kept for the panel's life so the Add host dialog it opens can return
    /// focus to it by name when it closes.
    add_focus: FocusHandle,
    /// Redraws the panel when its top line changes (a setup refused after the user moved on, a refused "don't ask
    /// again"): that line lives in a global of the setup module, which notifies no view by itself.
    _lines: Subscription,
    /// The same for the removal module's lines (a Remove that was not started, a refused "don't ask again").
    _removal_lines: Subscription,
    /// The same for the uninstall module's removal notice, which shows at the top of the panel.
    _uninstall_notice: Subscription,
    /// The panel's hold on the ticker ([`Runs`]), for the current UI session only: a reset lets go of it.
    runs: PerSession<Runs>,
    /// Keeps what the UI keeps about each host's run ([`progress::follow_current`]) up to date with every change of the
    /// current store, whether or not the panel is drawn, so a run's clock starts when it is first pushed even while
    /// Hosts is closed, and starts it over on every change of UI session, a reset's included.
    _following: session::ObservingCurrent,
}

/// The panel's hold on the ticker for one UI session, dropped with that session: a reset lets go of the pulse and the
/// one-second clock, which the next draw takes again if it needs them. What the UI keeps about each run (its first
/// sight and fold, [`progress::Seen`]) is not here but in `progress`'s app global, since the attention list's progress
/// card counts the same elapsed time.
#[derive(Default)]
struct Runs {
    /// The shared pulse's ticks, held exactly while the panel is drawn in an active window and a row shows a run in
    /// progress (a setup or an Update, pending or running).
    pulse: Option<ticker::Pulsing>,
    /// The ticker's one-second clock, held exactly while the panel is drawn and a row shows a run's elapsed time
    /// (`progress`'s module docs say why it is this clock and not the pulse). It redraws the panel once a second,
    /// whatever the window's activity and the reduced-motion setting.
    clock: Option<ticker::Clock>,
}

impl HostsPanel {
    /// A panel with Details off. It reads everything else from the store while it draws, so it can be made before the
    /// connection or the first snapshot and needs nothing from them.
    pub fn new(cx: &mut Context<Self>) -> Self {
        HostsPanel {
            details: false,
            menu: None,
            toggles: HashMap::new(),
            reveal: None,
            scroll: ScrollHandle::new(),
            add_focus: cx.focus_handle().tab_stop(true),
            _lines: setup::observe_panel_lines(cx),
            runs: PerSession::default(),
            _following: session::observe_current(cx, |panel: &mut Self, cx| panel.follow_runs(cx)),
            _removal_lines: remove::observe_panel_lines(cx),
            _uninstall_notice: uninstall::observe_notices(cx),
        }
    }

    /// Record what the current store's host progress says about each host's run ([`progress::follow_current`]), now.
    /// Reaching the panel's ticker hold through [`PerSession::current`] here, on every change of the connection, is
    /// also what drops the pulse and the clock as soon as a reset replaces the UI session, whether or not the panel is
    /// drawn. The rail's state ([`status::follow`]) and the uninstall's removal notice ([`uninstall::follow`]) follow
    /// the same changes from here.
    fn follow_runs(&mut self, cx: &mut Context<Self>) {
        status::follow(cx);
        uninstall::follow(cx);
        self.runs.current(cx);
        progress::follow_current(cx);
    }

    /// The sidebar stopped drawing the panel (Sessions shows instead): drop the pulse and the one-second clock. Only a
    /// drawn panel needs them, and the panel cannot tell it is not drawn, since only drawing runs its code, so without
    /// this a pulse taken while Hosts showed would keep the ticker's loop waking ten times a second for a panel nobody
    /// sees (the ticker's rule: no view holds a pulse while it draws no pulsing mark), and the clock would keep its own
    /// loop waking every second. The sidebar calls it where it hides the panel; a reset needs no call
    /// ([`HostsPanel::follow_runs`]). The next draw takes what it needs again.
    ///
    /// It also tells the rail's state that Hosts no longer shows, so a setup or Update that fails from now on warns the
    /// host's rail avatar again ([`status`]). That state keeps its own copy of whether Hosts shows, so every path that
    /// hides the panel must call this, and every path that shows it [`HostsPanel::shown`]; one that forgets leaves
    /// warnings raised while Hosts shows, or never acknowledged when it opens.
    pub fn hidden(&mut self, cx: &mut App) {
        let runs = self.runs.current(cx);
        runs.pulse = None;
        runs.clock = None;
        status::closed(cx);
    }

    /// The sidebar started drawing the panel (Hosts shows instead of Sessions): acknowledge every host's setup or
    /// Update warning, and warn of none while it shows, since the panel's rows show the failure itself (SPEC.md: the
    /// warning shows "until Hosts is opened"). The sidebar calls it wherever it shows the panel, the attention list's
    /// Details included, as it calls [`HostsPanel::hidden`] wherever it hides it; a reset needs no call, since what the
    /// rail's state keeps belongs to one UI session and the sidebar shows Sessions after a reset. Every path that shows
    /// the panel must call it, a test's or a golden scene's included, since the rail's state keeps its own copy of
    /// whether Hosts shows.
    pub fn shown(&mut self, cx: &mut App) {
        status::opened(cx);
    }

    /// Bring `host`'s row into view with its menu toggle focused, for the press that took `mark`: the attention list's
    /// Details and its rows (SPEC.md: they open "Hosts with that host's row in view and its menu button focused"), which
    /// call it right after showing the panel. Nothing else changes: not the Details switch, no row opens, and no menu.
    ///
    /// The panel draws only once shown, and a toggle's focus handle is element state, so the move waits for the frames
    /// that draw the row: the first frame that draws its toggle after this call records where it is, and the frame after
    /// it moves focus there through the focus coordinator ([`focus::focus_control`] with `mark`), which scrolls the row
    /// list as far as the toggle needs. So a user who pressed or typed anything since `mark` keeps where they are. A
    /// frame that draws no row for `host` (it left the registry) drops the reveal for good, so a later host under the
    /// same id is never revealed for it. `_window` is unused: the move happens while the panel draws.
    pub fn reveal(&mut self, host: i64, mark: &Mark, _window: &mut Window, cx: &mut Context<Self>) {
        self.reveal = Some((host, *mark));
        cx.notify();
    }

    /// The reveal this frame makes, if any, once [`HostsPanel::toggles`] has recorded this frame's toggles; `before` is
    /// the pending host's toggle handle as the last frame drew it. The toggle is focused only once a frame before this
    /// one drew the same handle, since that frame's layout is where the coordinator reads how far to scroll; until then
    /// the reveal waits one more frame, which it asks for.
    fn take_reveal(
        &mut self,
        before: Option<FocusHandle>,
        cx: &mut Context<Self>,
    ) -> Option<(FocusHandle, Mark)> {
        let (host, mark) = self.reveal.take()?;
        let (_, now) = self.toggles.get(&host)?;
        if before.as_ref() == Some(now) {
            return Some((now.clone(), mark));
        }
        self.reveal = Some((host, mark));
        cx.notify();
        None
    }

    /// Fold `host`'s run `run_id`, as its Collapse does: the row closes unless something else holds it open, and the
    /// inline status stays.
    fn fold(&mut self, host: i64, run_id: &str, cx: &mut Context<Self>) {
        progress::fold_current(host, run_id, cx);
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
        if !self.toggles.contains_key(&host) {
            return;
        }
        if let Some((open, layer)) = self.menu.take()
            && open == host
            && layers::is_open(layer, window, cx)
        {
            layers::close(layer, window, cx);
            cx.notify();
            return;
        }
        self.open_row_menu(host, MenuStart::First, window, cx);
    }

    /// Open `host`'s menu beside its row toggle with focus on the command `start` names: a click's or Down's first, or
    /// Up's last ([`open_menu`]). Does nothing for a row not drawn last frame. Down and Up reach the toggle only while
    /// its menu is closed, since the open menu holds focus.
    fn open_row_menu(
        &mut self,
        host: i64,
        start: MenuStart,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some((anchor, opener)) = self.toggles.get(&host).cloned() else {
            return;
        };
        let mark = Mark::take(cx);
        self.menu =
            open_menu(host, start, anchor, &opener, &mark, window, cx).map(|layer| (host, layer));
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

/// Whether `host`'s row is open: the Details switch is on, a setup or an uninstall is running on the host that the user
/// has not folded (`progress`, and what the panel keeps about it, `seen`), the host's last provisioning run failed, the
/// last work on the host was refused (`refused`), or the host is the helm's own machine with no supervisor running. All
/// but the switch open the row by themselves: a running setup or uninstall once, so its steps show as they go (SPEC.md:
/// "the running row expands once"), a failure always, so it shows its evidence and remedies whatever was folded, and
/// the local host without a supervisor so it shows what to do rather than reading as a phantom unreachable host
/// (SPEC.md, "Topology"). Each closes it again once it no longer holds: success ends the first, a new run or new work
/// on the host replaces the next two, and a supervisor starting ends the last.
pub(crate) fn disclosed(
    details: bool,
    host: &HostView,
    progress: Option<&ProvisioningView>,
    seen: Option<&progress::Seen>,
    refused: bool,
) -> bool {
    details
        || matches!(host.state, HostStateView::OwnedElsewhere { .. })
        || progress::opens_row(progress, seen)
        || offers::failed_operation(progress).is_some()
        || refused
        || offers::needs_app_supervisor(host)
}

/// What one row is drawn from, borrowed from the store and the operation store for the frame being drawn, with what the
/// panel keeps about its run and the inline status worked out from them.
struct RowInput<'a> {
    host: &'a HostView,
    progress: Option<&'a ProvisioningView>,
    refusal: Option<&'a Failure>,
    seen: Option<&'a progress::Seen>,
    status: Option<progress::Status>,
    sign_in: Option<crate::sign_in::Status>,
    /// Whether the host's Update is available ([`update::available`]), which the inline Update needs.
    updatable: bool,
    /// The host's lifecycle offer ([`offers::Offers::lifecycle`]), which the inline Resume needs.
    lifecycle: Option<(offers::Lifecycle, bool)>,
    /// The viewer's time zone for this frame, for the times the row's details give.
    zone: Zone,
    /// How long ago Farhelm asked the provider to change the host's machine, which its status gives after the changing
    /// phase's words ([`phase::asked_ago`]); `None` unless it is changing.
    asked_ago: Option<String>,
}

impl Render for HostsPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        render_count::record("hosts-panel", cx);
        let palette = theme::palette(cx);
        let weak = cx.weak_entity();
        // The pending reveal's toggle as the last frame drew it, read before this frame's replace them.
        let before = self
            .reveal
            .and_then(|(host, _)| self.toggles.get(&host).map(|(_, focus)| focus.clone()));
        let mut toggles = self.toggles(window, cx);
        let reveal = self.take_reveal(before, cx);
        let operations = session::current_operations(cx);
        let now = ticker::wall_now(cx);
        let zone = local_time::zone(cx);
        // Copied out for the frame: the rows borrow the store. A handful of entries at most, one per host with a run.
        let seen = progress::seen_now(cx);
        // The running pulse's opacity now, which every row's setup dot and running step share.
        let pulse = pulse_opacity(
            self.runs
                .current(cx)
                .pulse
                .as_ref()
                .map_or(0, |pulse| pulse.step(cx)),
            palette.status_pulse_min,
        );
        // Whether a row shows a run in progress (for the pulse), and whether one shows a run's elapsed time (for the
        // one-second clock): a pending "Setting up…" pulses but has no time to count.
        let mut pulsing = false;
        let mut counting = false;
        let mut status = None;
        let mut drawn: Vec<AnyElement> = Vec::new();
        // `None` until the hosts collection arrived: the count is not known yet.
        let mut count = None;
        // Whether any host's Update is available now, which makes Update all available.
        let mut any_updatable = false;
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
                        let available = update::available(host, progress, in_flight.is_some());
                        let lifecycle =
                            offers::offers(host, progress, in_flight.is_some()).lifecycle;
                        any_updatable |= available;
                        let asked_ago = phase::asked_ago(host, now);
                        // A changing machine's dot is its spinner, for as long as the pushed phase says it changes.
                        pulsing |= status.is_some() || asked_ago.is_some();
                        counting |= matches!(status, Some(progress::Status::Running { .. }))
                            || asked_ago.is_some();
                        let input = RowInput {
                            host,
                            progress,
                            refusal: operations.and_then(|operations| operations.failure(&op_key)),
                            seen,
                            status,
                            sign_in: crate::sign_in::status(host, cx),
                            updatable: available,
                            lifecycle,
                            zone,
                            asked_ago,
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

        self.keep_time(pulsing, counting, window, cx);
        // `.hosts-heading-actions`: Update all at the heading's end, the free room split around the Details switch, as
        // both carried `margin-left: auto`.
        let update_all = Button::new(
            "hosts-update-all",
            "Update all",
            HoverHelp::tooltip("Update all: update every remote host that has an update available")
                .below(),
            Role::Button,
        )
        .tier(Tier::Neutral)
        .compact()
        .disabled(!any_updatable)
        .on_click(|_, window, cx| {
            // Every host the registry lists at the press; `update::start_all` decides there which of them it updates.
            let mark = Mark::take(cx);
            let listed: Vec<i64> = session::current_store(cx)
                .map(|store| {
                    let collections = store.read(cx).collections();
                    collections
                        .ordered::<HostView>()
                        .iter()
                        .map(|host| host.id)
                        .collect()
                })
                .unwrap_or_default();
            update::start_all(&listed, mark, window, cx);
        });
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
                        ).below(),
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
            // The old actions group (`.hosts-heading-actions`): Update all and the add button at the heading's end, 4
            // pixels apart, its automatic margin beside the switch's, as the old sheet gave both, so the two share the
            // heading's free space. A button takes no margin of its own, so this holder carries it.
            .child(
                div()
                    .ml_auto()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(ACTIONS_GAP))
                    .child(update_all)
                    .child(add_button(&self.add_focus)),
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
        // The removal notice of an uninstall this window confirmed, once its host has left the list: no row is left to
        // show it under.
        let uninstalled = uninstall::notice(cx);
        if let Some((toggle, mark)) = reveal {
            // Made while this frame draws, so the toggle is focused in it and the row list scrolls before its layout.
            focus::focus_control(&toggle, &mark, window, cx);
        }
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
            .children(uninstalled)
            .child(focus::scroll_region(
                div()
                    .id("host-list")
                    .role(Role::List)
                    .aria_label("Hosts")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .child(div().flex().flex_col().children(drawn)),
                &self.scroll,
            ))
    }
}

impl HostsPanel {
    /// Hold the pulse while the window is active and a row shows a run in progress (`pulsing`), and the one-second
    /// clock while a row shows a run's elapsed time (`counting`), whatever the window's activity. The ticker's rule for
    /// the pulse: never held for a window nobody is looking at, where it would keep the tick loop firing for nothing.
    /// The clock has no such rule, since a count must still be right when the user looks back at the window.
    fn keep_time(
        &mut self,
        pulsing: bool,
        counting: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let pulses = pulsing && ticker::is_active(window, cx);
        let runs = self.runs.current(cx);
        if pulses != runs.pulse.is_some() {
            runs.pulse = pulses.then(|| ticker::pulse(window, cx));
        }
        if counting != runs.clock.is_some() {
            runs.clock = counting.then(|| ticker::every_second(cx));
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
                let (for_click, for_arrow) = (cx.weak_entity(), cx.weak_entity());
                let toggle = menu::toggle(
                    id,
                    &name,
                    open == Some(id),
                    &parts,
                    move |window, cx| {
                        let _ = for_click.update(cx, |panel, cx| panel.toggle_menu(id, window, cx));
                    },
                    move |start, window, cx| {
                        let _ = for_arrow
                            .update(cx, |panel, cx| panel.open_row_menu(id, start, window, cx));
                    },
                );
                self.toggles.insert(id, parts);
                (id, toggle)
            })
            .collect()
    }

    /// One host's row (`.host-row`): its header line ending in its menu `toggle` (with the inline status of a run in
    /// progress under it), then its details while it is open, then the last refusal of work on it, which shows open or
    /// not, then a run's steps while it is open. `twin` names a host by id for a duplicate's details; `panel` is
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
        let sign_in = input
            .sign_in
            .map(|state| crate::sign_in::action(host, state, format!("host-sign-in-{id}"), false));
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
        let line = line.children(sign_in);
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
        let main = match (input.sign_in, &input.status) {
            (Some(state), _) => main.child(
                line.child(sign_in_status(id, state, palette))
                    .children(toggle),
            ),
            // A Suspend's or Resume's request, and the provider's change it starts (the pushed phase, counting the
            // seconds since Farhelm asked), put the status on a line of its own under the name, as the old row's grid
            // moved `.host-status` to its second row beside `.host-lifecycle-progress`; the unavailable inline Resume
            // stays on the name's line before the toggle, as the old grid kept `.host-resume` there.
            (None, Some(status @ progress::Status::Lifecycle(_))) => {
                let opener = self.toggles.get(&id).map(|(_, focus)| focus.clone());
                main.child(
                    line.children(lifecycle::button(host, input.lifecycle, opener))
                        .children(toggle),
                )
                .child(
                    div()
                        .pl(px(MARK + ROW_GAP))
                        .pr(px(DETAIL_PAD_X))
                        .min_w_0()
                        .child(progress::status_mark(id, &name, status, pulse, palette)),
                )
            }
            (None, None) if input.asked_ago.is_some() => {
                let opener = self.toggles.get(&id).map(|(_, focus)| focus.clone());
                main.child(
                    line.children(lifecycle::button(host, input.lifecycle, opener))
                        .children(toggle),
                )
                .child(
                    div()
                        .pl(px(MARK + ROW_GAP))
                        .pr(px(DETAIL_PAD_X))
                        .min_w_0()
                        .child(phase_status(
                            host,
                            input.asked_ago.as_deref(),
                            pulse,
                            palette,
                        )),
                )
            }
            (None, Some(status)) => main.child(line.children(toggle)).child(
                div()
                    .pl(px(MARK + ROW_GAP))
                    .pr(px(DETAIL_PAD_X))
                    .min_w_0()
                    .child(progress::status_mark(id, &name, status, pulse, palette)),
            ),
            (None, None) => {
                let opener = self.toggles.get(&id).map(|(_, focus)| focus.clone());
                main.child(
                    // The inline Resume sits between the name and the phase, as the old row drew it.
                    line.children(lifecycle::button(host, input.lifecycle, opener.clone()))
                        .child(phase_status(
                            host,
                            input.asked_ago.as_deref(),
                            pulse,
                            palette,
                        ))
                        .children(update::button(host, input.updatable, opener))
                        .children(toggle),
                )
            }
        };
        let lines: Vec<AnyElement> = if open {
            phase::detail_lines(host, twin, input.zone)
                .into_iter()
                .map(|(kind, line)| detail_line(kind, line, palette))
                .collect()
        } else {
            Vec::new()
        };
        // The run's steps under an open row, with Collapse while the row is open for the run alone and the user may
        // fold it (with Details on, the switch holds every row open, so there is nothing to fold).
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
        HoverHelp::tooltip(add::ADD_HOST_HELP).below(),
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

/// Host `host` as the registry lists it now, or `None` when the window is inert or it is not listed: how this module's
/// actions find the host a press or a reply is about, by id, at that moment. One copy for every submodule that needs
/// it.
fn listed(host: i64, cx: &App) -> Option<HostView> {
    let store = session::current_store(cx)?;
    let collections = store.read(cx).collections();
    collections.get::<HostView>(&HostView::key(host)).cloned()
}

/// The entity id of the current UI session's operation store; `None` while the window is inert. What this module's app
/// globals (the Hosts panel's lines in [`setup`] and [`remove`], the rail's state in [`status`], the removal notices in
/// [`uninstall`]) tag what they keep with, and compare on every read: a reset makes a new operation store, so whatever
/// was kept under the old one stops showing, as everything from the helm does after a reset. Private to this module and
/// its submodules, which own those globals.
fn current_operations_id(cx: &App) -> Option<EntityId> {
    session::current_operations(cx).map(|operations| operations.entity_id())
}

/// Whether a session launch is in flight anywhere in the operation store: what this module's host actions (Add host's
/// probe, a setup's submission, a row's Re-run, a host's removal, a host's Update and Update all, its uninstall, and
/// its Retry and Adopt) refuse to start during. The old app refused a host change during a launch and
/// the reverse, through one page-wide token; the migration's orchestrator decided on 2026-11-14 that each side refuses
/// its own action instead, with a reason, while the other's work is in flight in the operation store. What counts as a
/// launch is mostly the operation store's to say ([`Work::creates_a_session`]: the launcher's New and Clone, and a
/// session's Restart and Replace), besides the environment create below. `false` in an inert window, which has no
/// store and starts nothing anyway.
///
/// An environment create the launcher still waits on counts too (`launcher::launch::environment_create_running`): its
/// operation key clears on the helm's 202, while the launch goes on for minutes, so the launcher's wait for it is what
/// keeps launches and host-registry changes exclusive for the whole create (SPEC.md, "Launches and changes to the host
/// registry still exclude each other").
pub(crate) fn launch_in_flight(cx: &App) -> bool {
    crate::launcher::launch::environment_create_running(cx)
        || session::current_operations(cx)
            .is_some_and(|operations| operations.read(cx).any_in_flight(Work::creates_a_session))
}

/// Whether a host-registry change is in flight anywhere in the operation store: what refuses the launcher's Launch and
/// a fresh New, and the card's open of an agent's Add host. The other half of [`launch_in_flight`]'s exclusion, which
/// the migration's orchestrator decided on 2026-11-14 is kept by each side refusing its own action while the other's
/// work is in flight, rather than by the old app's shared token. `false` in an inert window.
///
/// What counts is the operation store's own classification ([`Work::changes_host_registry`]): a host's probe and setup
/// (a new host's, or a registered one's re-run), its removal and its Update. The orchestrator decided on 2026-11-14
/// that the probe counts, which the old app left outside its token, and that a save of a registered host's settings
/// does not, its destination and alias included: that window is one short save, so a launch does not wait for it (a
/// destination save refuses itself during a launch instead: [`settings::Field::waits_for_launches`]). Suspend and
/// Resume are not registry changes (SPEC.md), and nothing done to a session is.
pub(crate) fn host_change_in_flight(cx: &App) -> bool {
    session::current_operations(cx).is_some_and(|operations| {
        operations
            .read(cx)
            .any_in_flight(Work::changes_host_registry)
    })
}

/// A host's identity mark in its row: its chosen shape and color at 12 pixels, a status named "Local" or "Remote" for
/// assistive technology, or for a sandbox or container its provider's word ("Sandbox", "Container"), with hover help
/// saying what that means for the machine. The session rows in that environment use the same word, so the panel and
/// those rows agree on what the machine is; their help describes a session in it instead. It never gives way to a long
/// name.
fn identity_mark(host: &HostView, locality: Locality, palette: &Palette) -> AnyElement {
    let (word, help) = match locality {
        Locality::Local => ("Local", "Local: on this machine, where the helm runs"),
        Locality::Remote => ("Remote", "Remote: on another machine, reached over SSH"),
        Locality::Environment(provider) => {
            let words = ProviderUi(provider);
            (words.locality_label(), words.host_mark_help())
        }
    };
    div()
        .flex_none()
        .child(
            StatusMark::new(
                SharedString::from(format!("host-mark-{}", host.id)),
                word,
                HoverHelp::tooltip(help).beside(),
                Role::Image,
                word,
            )
            .content(host_mark(host.icon, host.color, px(MARK), palette)),
        )
        .into_any_element()
}

/// A host's phase (`.host-status`): the dot and the words in the phase's tone, the words cut short before the toggle's
/// place leaves the row. Named by its words; its hover is the words, or for a host running a newer Farhelm both
/// versions and the remedy, which is then also its description, as the old status described only Too new. A changing
/// sandbox or container adds `asked_ago` after its words ("asked Tensorlake 12 s ago"), outside its name, as the old
/// elapsed was hidden from assistive technology, wrapping under the words where the line is too narrow; and its dot
/// pulses at `pulse`, the old row's spinner (SPEC.md, "Topology": "with a spinner"), still under reduced motion, where
/// the pulse stays full.
fn phase_status(
    host: &HostView,
    asked_ago: Option<&str>,
    pulse: f32,
    palette: &Palette,
) -> AnyElement {
    let words = phase::words(&host.state);
    // The host's tone, which turns amber for failed forwards beside a working connection ([`phase::host_tone`]).
    let tone = phase::host_tone(host);
    let ink = tone_ink(tone, palette);
    // `min_w_0` lets the status shrink below its words, so they are cut short rather than pushed out of the row.
    div()
        .min_w_0()
        .child(
            StatusMark::new(
                SharedString::from(format!("host-status-{}", host.id)),
                words,
                HoverHelp::tooltip(phase::hover(&host.state)).beside(),
                Role::Status,
                words,
            )
            .ink(ink)
            .described(offers::runs_newer_version(&host.state))
            .content(
                phase_dot_and_words(
                    &host.state,
                    tone,
                    if asked_ago.is_some() { pulse } else { 1. },
                    palette,
                )
                // The count wraps under the words where the line is too narrow for both, as the old
                // `.host-lifecycle-progress` wrapped, and is cut short only when it alone does not fit.
                .when(asked_ago.is_some(), |line| line.flex_wrap())
                // `.host-lifecycle-elapsed`: subdued and a size smaller than the words.
                .children(asked_ago.map(|ago| {
                    div()
                        .min_w_0()
                        .truncate()
                        .text_size(px(11.))
                        .text_color(theme::color(palette.fg_2))
                        .child(ago.to_owned())
                })),
            ),
        )
        .into_any_element()
}

/// The live question may outrank the phase for the row label while an older ownership refusal stays in its details.
fn sign_in_status(id: i64, state: crate::sign_in::Status, palette: &Palette) -> AnyElement {
    let words = state.words();
    div()
        .min_w_0()
        .child(
            StatusMark::new(
                format!("host-status-{id}"),
                words,
                HoverHelp::tooltip(words).beside(),
                Role::Status,
                words,
            )
            .ink(palette.warn),
        )
        .into_any_element()
}

/// A phase's dot and words in its tone's ink, on one line, the words cut short when they do not fit: what a row's
/// status and the host settings' header both show (the old `.host-status` span, which both drew). The host rail's
/// connection dot takes its ink from the same tone ([`tone_ink`]), so the rail and the panel cannot disagree on a
/// phase's color; change them together. `tone` is the phase's, or the host's where the caller knows the host
/// ([`phase::host_tone`]). The dot is drawn at `pulse` opacity, 1 for a still one.
fn phase_dot_and_words(state: &HostStateView, tone: Tone, pulse: f32, palette: &Palette) -> Div {
    let ink = tone_ink(tone, palette);
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
                .bg(theme::color(ink))
                .when(pulse < 1., |dot| dot.opacity(pulse)),
        )
        .child(div().min_w_0().truncate().child(phase::words(state)))
}

/// The ink of a phase's tone (the old `.host-status.<class>` colors). The host rail's connection dot shares it (the
/// old `.host-rail-state.<class>` rules gave the same colors), so a phase reads the same on the rail as in the panel.
pub(crate) fn tone_ink(tone: Tone, palette: &Palette) -> Token {
    match tone {
        Tone::Connected => palette.ok,
        Tone::Advisory => palette.warn,
        Tone::Pending | Tone::Quiet => palette.fg_1,
        Tone::Attention => palette.danger_strong,
    }
}

/// One detail line (`.host-detail`, `.host-destination-detail`, `.host-remedy`): 12-pixel text, the remedy in the
/// ordinary ink and the rest in the quiet one. The sentence is the line's one labelled node for assistive technology;
/// this wrapper adds only the line's spacing and ink, which the sentence inherits.
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
