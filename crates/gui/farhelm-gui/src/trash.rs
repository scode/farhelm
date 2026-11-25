//! The session list header's Trash button, the archived-checkout reads behind its count, and the Trash dialog it opens.
//!
//! SPEC.md, "Managed checkouts" (the trash paragraphs, which also have the session list's header place Trash before
//! Templates and New) and "Session list" (Trash with its count among the compact icon buttons beside the title). Ported
//! from the old app's `list/trash.rs` at reference commit `24bc0580` (`TrashControl` and `HostTrashReader`); the dialog
//! is [`dialog`], and the plain rules both use are [`listing`]. Here's the TLDR of how it behaves:
//!
//! - **A view of its own, placed by the list.** [`TrashButton`] keeps the button's focus handle, this window's reads of
//!   each host's trash and the dialog it opened; the session list creates it once and draws it in its header, as the
//!   rail draws the help menu's `?` button. Its state is held per UI session ([`PerSession`]), so a reset drops every
//!   read, recheck and kept refusal with the session.
//! - **The count is event-driven.** The helm pushes each host's `trash_epoch`, which it advances whenever that host's
//!   trash changes (a trash delete here, or a session Delete that archived a checkout, from this window, the command
//!   line or an agent). The button follows the store ([`session::observe_current`]) and reads a connected host's trash
//!   (`POST /api/checkout-trash/list`, a keyless read: [`operations::send`]) when it first sees the host's connection,
//!   and again whenever the epoch moves. Ordinary pushes and idle hosts cause no read; while the registry itself cannot
//!   be read, nothing is read or dropped.
//! - **Only hosts freeing space are rechecked.** A host whose last listing reported active removal is read again every
//!   [`RECHECK`] ([`ticker::after`]) until a successful listing says removal ended. A failed read keeps the last
//!   report, so an outage never looks like the space was freed (SPEC.md: "A failed read retains the last removal
//!   report"), and ends the rechecks until the host's trash epoch moves or it reconnects ([`TrashButton::listed`] says
//!   why).
//! - **The can.** An outline can when empty, an open-lid can and the count when reachable hosts hold checkouts. While
//!   a delete of this window's is in flight or a host reports freeing space, the can turns upside down and bobs, a step
//!   at a time from the ticker ([`ticker::animate`]), and stays upside down and still under reduced motion, where no
//!   ticks come. The hover help names the hosts freeing space and, while a refusal waits in the dialog, says to open
//!   the trash to see why.
//! - **Deletes are work on a host's trash key.** An entry's Delete or Retry in the dialog, and each host's share of a
//!   confirmed Delete all, starts [`operations::Work::TrashDelete`] under [`OpKey::trash`], never [`OpKey::host`], so it
//!   does not block the host's other actions; the frozen command names the checkouts and the connection they were
//!   listed on. Its answer marks what a capable host accepted as removing, keeps refusals for the dialog, and reads the
//!   host again.
//! - **[`TrashButton::open`] is the entry point** for the dialog, with the return target the caller names: the
//!   button's own press names the button, and the rail notices panel's Open Trash names the rail's bell
//!   ([`crate::rail_notices`]). That panel's Retry now goes through [`TrashButton::retry`], the dialog's Retry with the
//!   panel's own availability rule ([`Snapshot::retry_target`]).
//! - **The archive cue** ([`cue`]): the can wiggles when this window's session Delete moved a checkout into the trash.
//! - **A staged run's hosts** (agent builds with a pins file only): a staged host's listing comes from the pins file
//!   instead of a read, through the same answer path, and a delete from it is refused before anything is sent, with a
//!   refusal beside each entry ([`crate::agent_startup::staged_trash`]): its entries exist on no host.
//!
//! NOTE: The old page's read generations are not ported, and its uncertain-outcome bookkeeping only in a smaller form
//! ([`listing`]'s module docs say why).
//!
//! NOTE: Deviations from the old button, both from what the GPUI app does not have: the old button was disabled while
//! the page's launch-and-registry lock was held, a lock this app does not have (launches here are each on a key of their
//! own), so the button is always available and the per-host guard is the host's own key instead; and the old app read
//! every host again, without sizes, when the dialog closed, which nothing in SPEC.md asks for and is not done here. A
//! failed read also ends a host's rechecks, where the old page kept polling and retried the read after half a second
//! ([`TrashButton::listed`] says why).

pub(crate) mod cue;
pub(crate) mod dialog;
pub(crate) mod listing;

#[cfg(test)]
pub(crate) mod tests;

use std::collections::HashMap;
use std::f32::consts::PI;
use std::rc::Rc;
use std::time::Duration;

use farhelm_helm_client::{ApiPath, Method};
use farhelm_proto::CheckoutTrashDeleted;
use farhelm_proto::ui_channel::{Availability, Collection, HostView};
use gpui::{
    App, AppContext as _, Context, FocusHandle, FontWeight, IntoElement, ParentElement as _,
    Render, Styled as _, Transformation, WeakEntity, Window, div, point, px, svg,
};
use serde_json::json;

use crate::assets::Icon;
use crate::components::{Button, HoverHelp, Role, Tier};
use crate::focus::{Mark, ReturnTo};
use crate::host_text::HostText;
use crate::layers::{self, Dialog, LayerId};
use crate::operations::{self, Command, NotStarted, OpKey, Outcome as Answer, Settled, Work};
use crate::session::{self, PerSession};
use crate::theme;
use crate::ticker;
use dialog::TrashDialog;
use listing::{HostSnapshot, HostTrash, ListReply, Notice, Snapshot, Target};

/// How long a host that reported active removal waits before it is read again: the old app's three seconds.
pub(crate) const RECHECK: Duration = Duration::from_secs(3);

/// The button's element id, the old `.checkout-trash-button`.
pub(crate) const BUTTON_ID: &str = "checkout-trash-button";

/// The status the helm answers a trash delete with when it stopped waiting on the host (its host-wait bound) while the
/// cleanup may still go on ([`TrashButton::deleted`]).
const HOST_TIMEOUT: u16 = 504;

/// Why a confirmed delete sent nothing ([`TrashButton::delete_batches`]), which the dialog shows until its next question.
/// The old band said nothing in either case, so neither line ports one of its own.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Refused {
    /// Another trash delete of this window's runs, or a confirmed host's own key is busy.
    Busy,
    /// The registry cannot be read, or a confirmed host left the connection its batch was listed on.
    Connection,
}

impl Refused {
    /// The line the dialog shows. A busy refusal gives the operation store's shared reason after a lead-in naming what
    /// did not happen (SPEC_impl.md, "Work that finishes later"); that reason is in the past tense because the line
    /// stays after the work in the way has finished. A stale connection keeps the old dispatch's words for a frozen
    /// selection that is no longer current (`refuse` in `list/trash.rs` at reference commit `24bc0580`, which showed
    /// them only to the rail's Retry).
    pub(crate) fn words(self) -> String {
        match self {
            Refused::Busy => format!("Not deleted: {}", NotStarted::BUSY_REASON),
            Refused::Connection => {
                "The host connection changed. Wait for Trash to refresh before retrying.".to_owned()
            }
        }
    }
}

/// The list route, as a refusal without words names it.
const LIST_SHOWN: &str = "POST /api/checkout-trash/list";

/// The delete route, as a refusal without words names it.
const DELETE_SHOWN: &str = "POST /api/checkout-trash/delete";

/// What a delete from a staged run's trash says, in place of the host's answer it never asks for ([`TrashButton::send`]).
#[cfg(any(feature = "agent", test))]
const STAGED_DELETE_REFUSAL: &str =
    "Not deleted: this trash is staged for screenshots, and its checkouts exist on no host";

/// The can's size in the button, the old `.checkout-trash-button svg`.
const CAN: f32 = 16.;

/// The button's content box: the heading's 24-pixel button less its 1-pixel edge and the heading padding's 2 pixels,
/// above and below.
const CONTENT_HEIGHT: f32 = 18.;

/// `.session-heading .checkout-trash-button`'s `padding-inline: 5px`, drawn inside the content box, since the heading
/// padding has none at the sides.
const CONTENT_PAD_X: f32 = 5.;

/// The count's text, the old `.checkout-trash-count`.
const COUNT_TEXT: f32 = 11.;

/// How far the upside-down can sits above its place at each of the bob's eight steps. The old `trash-delete-bob`
/// keyframes went from 0 to 3 pixels up and back in one second under `steps(4)`, which steps per keyframe interval: four
/// steps up from 0, four down from 3.
const BOB: [f32; 8] = [0., 0.75, 1.5, 2.25, 3., 2.25, 1.5, 0.75];

/// The bob's step after `ticks` ticks: eight steps a second over the ticker's ten ticks, so a step lasts one tick or
/// two in turn, keeping the old one-second cycle.
pub(crate) fn bob_lift(ticks: u64) -> f32 {
    BOB[(ticks * BOB.len() as u64 / u64::from(ticker::TICKS_PER_SECOND)) as usize % BOB.len()]
}

/// The Trash button (see the module docs). Created once by the session list, which draws it in its header for the
/// window's life.
pub struct TrashButton {
    /// The button's focus handle, a tab stop, where closing the dialog returns focus when the button opened it.
    focus: FocusHandle,
    /// This UI session's reads and kept refusals.
    current: PerSession<Kept>,
    /// Reads and drops hosts' trash as the store changes ([`TrashButton::sync`]).
    _following: session::ObservingCurrent,
    /// The bob's ticks, held while the can is upside down and the window active, like the session list's pulse.
    bob: Option<ticker::AnimationTicks>,
    /// The archive cue's wiggle ([`cue`]): its ticks while it plays.
    wiggle: Option<ticker::AnimationTicks>,
    /// The drawn-button token ([`Self::shown`]) the cue saw when it asked for a wiggle, which the next draw starts if it
    /// still draws that same button: starting one needs the window, which the cue's caller does not have, and a button
    /// that stopped being drawn in between must not play late.
    wiggle_asked: Option<gpui::EntityId>,
    /// State that lives exactly as long as the button is drawn in consecutive frames (GPUI's keyed element state), so
    /// the cue can tell a drawn button from one the list is not showing.
    shown: WeakEntity<()>,
    /// The dialog this button opened last. Its view going away (the layer closed, or a reset closed every layer) is
    /// how the button tells whether it is open without a window.
    dialog: Option<(LayerId, WeakEntity<TrashDialog>)>,
}

/// What the button keeps for one UI session.
#[derive(Default)]
struct Kept {
    /// Each connected host's trash, by registry id.
    hosts: HashMap<i64, HostTrash>,
    /// Each host's cleanup outcomes the dialog keeps showing, by registry id. Kept across disconnection and a failed
    /// registry read; dropped once a successful registry read no longer lists the host (SPEC.md).
    notices: HashMap<i64, Vec<Notice>>,
    /// How many reads this session has sent: the source of every read's number ([`HostTrash::serial`]). One counter
    /// for all hosts, never restarted while the session lasts, so a host's entry made again for a new connection never
    /// reuses a number an answer from its old connection still carries.
    reads: u64,
}

impl TrashButton {
    /// A button following the current UI session; it reads every connected host's trash once the first snapshot is in.
    pub fn new(cx: &mut Context<Self>) -> Self {
        cue::register(cx.weak_entity(), cx);
        TrashButton {
            focus: cx.focus_handle().tab_stop(true),
            current: PerSession::default(),
            _following: session::observe_current(cx, |button: &mut Self, cx| button.sync(cx)),
            bob: None,
            wiggle: None,
            wiggle_asked: None,
            shown: WeakEntity::new_invalid(),
            dialog: None,
        }
    }

    // ---- Reads -------------------------------------------------------------------------------------------------------

    /// Bring the kept reads in line with the registry: drop hosts the registry no longer lists as connected (and the
    /// notices of hosts it no longer lists at all), and read each connected host that is new, reconnected, or whose
    /// `trash_epoch` moved, or that a staged run started or stopped answering for (its alias changed). Does nothing while the registry is unread or unavailable.
    fn sync(&mut self, cx: &mut Context<Self>) {
        let Some(store) = session::current_store(cx) else {
            return;
        };
        let (registry, available) = {
            let collections = store.read(cx).collections();
            let registry: Vec<(i64, u64, u64, bool)> = collections
                .ordered::<HostView>()
                .into_iter()
                .map(|host| {
                    let connected = listing::connected(host);
                    (host.id, host.incarnation, host.trash_epoch, connected)
                })
                .collect();
            let available = collections
                .meta(Collection::Hosts)
                .is_some_and(|meta| meta.availability == Availability::Available);
            (registry, available)
        };
        if !available {
            return;
        }
        // Which hosts a staged run answers for, asked before the kept reads are borrowed (always none outside an agent
        // build with a pins file).
        let staged: Vec<i64> = registry
            .iter()
            .map(|host| host.0)
            .filter(|&id| crate::agent_startup::staged_trash(id, cx).is_some())
            .collect();
        let kept = self.current.current(cx);
        kept.notices
            .retain(|id, _| registry.iter().any(|host| host.0 == *id));
        kept.hosts
            .retain(|id, _| registry.iter().any(|host| host.0 == *id && host.3));
        let mut due = Vec::new();
        for &(id, incarnation, epoch, connected) in &registry {
            if !connected {
                continue;
            }
            let entry = kept.hosts.entry(id).or_default();
            if entry.incarnation != incarnation {
                *entry = HostTrash {
                    incarnation,
                    ..HostTrash::default()
                };
            }
            let staged = staged.contains(&id);
            if entry.serial == 0 || entry.epoch != epoch || entry.staged != staged {
                entry.epoch = epoch;
                entry.staged = staged;
                due.push(id);
            }
        }
        for id in due {
            self.read(id, cx);
        }
        cx.notify();
    }

    /// Read `host`'s trash, with sizes while the dialog is open. The read becomes the host's newest; an answer to any
    /// other read is dropped when it comes. Any pending recheck is dropped, since this read is it.
    fn read(&mut self, host: i64, cx: &mut Context<Self>) {
        let measure = self.measuring();
        let button = cx.weak_entity();
        let Ok(path) = ApiPath::api(&["checkout-trash", "list"]) else {
            return;
        };
        let kept = self.current.current(cx);
        let Some(entry) = kept.hosts.get_mut(&host) else {
            return;
        };
        kept.reads += 1;
        entry.serial = kept.reads;
        entry.loading = true;
        entry.recheck = None;
        let serial = entry.serial;
        // A staged run's host answers from the pins file instead of being read (`agent_startup`, "Staged host
        // values"), through the same answer path a read's reply takes, so the can, the count and the dialog draw it as
        // they would a host's.
        if let Some(listing) = crate::agent_startup::staged_trash(host, cx) {
            let reply = ListReply {
                listing,
                notice_error: None,
            };
            self.listed(host, serial, Ok(reply), cx);
            return;
        }
        let body = json!({
            "host": host,
            "expected_incarnation": entry.incarnation,
            "measure_sizes": measure,
        });
        let sent = operations::send_unordered(
            Method::Post,
            path,
            Some(body),
            LIST_SHOWN,
            cx,
            move |answer, cx| {
                let reply = operations::decode::<ListReply>(answer, LIST_SHOWN);
                let _ = button.update(cx, |button, cx| button.listed(host, serial, reply, cx));
            },
        );
        if !sent && let Some(entry) = self.current.current(cx).hosts.get_mut(&host) {
            entry.loading = false;
        }
        cx.notify();
    }

    /// Read `host` for every host kept, as the dialog opens: the sizes are measured then.
    fn measure(&mut self, cx: &mut Context<Self>) {
        // In id order, so the reads go out in the same order every time.
        let mut hosts: Vec<i64> = self.current.current(cx).hosts.keys().copied().collect();
        hosts.sort_unstable();
        for host in hosts {
            self.read(host, cx);
        }
    }

    /// The answer to read `serial` of `host`: kept if it is the host's newest. A listing replaces what was kept and
    /// says whether removal is active, and a host still reporting removal is rechecked after [`RECHECK`]. A failure
    /// replaces the listing with its words, keeps the last removal report (SPEC.md: "A failed read retains the last
    /// removal report") and schedules no recheck: the host is read again when its trash epoch moves, on a new
    /// connection, or when the dialog opens.
    ///
    /// DECISION (the other-dialogs area's coordinator, 2026-11-15): no recheck after any failed read. A read fails only
    /// when the host or its connection is in trouble. A timeout still spends the host's full 20-second reply bound.
    /// Keyless reads carry the refusal's words, so the window treats quick failures the same way. A reconnect reads
    /// the host again.
    fn listed(
        &mut self,
        host: i64,
        serial: u64,
        reply: Result<ListReply, HostText>,
        cx: &mut Context<Self>,
    ) {
        let button = cx.weak_entity();
        let Some(entry) = self.current.current(cx).hosts.get_mut(&host) else {
            return;
        };
        if entry.serial != serial {
            return;
        }
        entry.loading = false;
        match reply {
            Ok(reply) => {
                entry.removing = reply.listing.removing;
                entry.notice_error = reply.notice_error.map(HostText::from_host);
                entry.listing = Some(Ok(Rc::new(reply.listing)));
            }
            Err(words) => {
                entry.listing = Some(Err(words));
                cx.notify();
                return;
            }
        }
        if entry.removing {
            entry.recheck = Some(ticker::after(RECHECK, cx, move |cx| {
                let _ = button.update(cx, |button, cx| button.recheck(host, cx));
            }));
        }
        cx.notify();
    }

    /// A recheck came due: read `host` again if it still reports removal and no read is out (the one out reschedules
    /// when it answers).
    fn recheck(&mut self, host: i64, cx: &mut Context<Self>) {
        let due = self
            .current
            .current(cx)
            .hosts
            .get(&host)
            .is_some_and(|entry| entry.removing && !entry.loading);
        if due {
            self.read(host, cx);
        }
    }

    // ---- What is drawn -----------------------------------------------------------------------------------------------

    /// Everything the button and the dialog draw, from the kept reads, the store's registry and the operation store's
    /// work at this moment.
    pub(crate) fn snapshot(&mut self, cx: &App) -> Snapshot {
        let Some(store) = session::current_store(cx) else {
            return Snapshot::default();
        };
        let operations = session::current_operations(cx);
        let collections = store.read(cx).collections();
        let meta = collections.meta(Collection::Hosts);
        let hosts_known = meta.is_some_and(|meta| meta.availability == Availability::Available);
        let hosts_error = match meta.map(|meta| &meta.availability) {
            Some(Availability::Unavailable { error }) => Some(HostText::from_host(error.as_str())),
            _ => None,
        };
        let in_flight = |key: &OpKey| {
            operations
                .as_ref()
                .and_then(|operations| operations.read(cx).in_flight(key))
        };
        let delete_in_flight = operations.as_ref().is_some_and(|operations| {
            operations
                .read(cx)
                .any_in_flight(|work| work == Work::TrashDelete)
        });
        let kept = self.current.current(cx);
        let mut deleting = None;
        let hosts = collections
            .ordered::<HostView>()
            .into_iter()
            .map(|view| {
                let key = HostView::key(view.id);
                if in_flight(&OpKey::trash(&key)).is_some() {
                    deleting = Some(view.id);
                }
                let connected = listing::connected(view);
                let state = kept
                    .hosts
                    .get(&view.id)
                    .filter(|state| connected && state.incarnation == view.incarnation);
                HostSnapshot {
                    id: view.id,
                    name: HostText::from_host(view.name.as_str()),
                    incarnation: view.incarnation,
                    connected,
                    listing: state.and_then(|state| state.listing.clone()),
                    pending: state.is_none_or(|state| state.loading || state.listing.is_none()),
                    removing: state.is_some_and(|state| state.removing),
                    notice_error: state.and_then(|state| state.notice_error.clone()),
                    notices: kept.notices.get(&view.id).cloned().unwrap_or_default(),
                    busy: in_flight(&OpKey::host(&key)).is_some(),
                }
            })
            .collect();
        Snapshot {
            hosts_known,
            hosts_error,
            hosts,
            delete_in_flight,
            deleting,
        }
    }

    /// How far up the can is drawn now, or `None` while the button holds no bob: for the tests of the bob.
    #[cfg(test)]
    pub(crate) fn lift(&self, cx: &App) -> Option<f32> {
        self.bob.as_ref().map(|bob| bob_lift(bob.count(cx)))
    }

    /// The dialog this button opened, while it is open: for the screenshot goldens, which turn its settle pause off.
    #[cfg(test)]
    pub(crate) fn dialog_view(&self) -> Option<gpui::Entity<TrashDialog>> {
        self.dialog
            .as_ref()
            .and_then(|(_, dialog)| dialog.upgrade())
    }

    /// Whether the dialog this button opened is still open: its view lives as long as its layer does.
    fn measuring(&self) -> bool {
        self.dialog
            .as_ref()
            .is_some_and(|(_, dialog)| dialog.upgrade().is_some())
    }

    // ---- The dialog and its deletes ----------------------------------------------------------------------------------

    /// Open the Trash dialog over everything else in `window`, measuring every connected host's trash as it opens,
    /// unless the user has moved on since `mark`, the window is inert, or it is open already. Closing it returns focus
    /// to `returns_to`: the button's own press names the button.
    pub fn open(
        &mut self,
        returns_to: ReturnTo,
        mark: &Mark,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let already = self
            .dialog
            .as_ref()
            .is_some_and(|(layer, _)| layers::is_open(*layer, window, cx));
        if already || session::current_store(cx).is_none() {
            return;
        }
        let button = cx.entity();
        let dialog = cx.new(|cx| TrashDialog::new(button, cx));
        let start = dialog.read(cx).start();
        let Some(layer) = layers::open_dialog(
            Dialog {
                view: dialog.clone().into(),
                container: cx.focus_handle(),
                // The footer's Close (SPEC.md: "Initial focus stays on footer Close").
                start,
                subject: None,
                // The dialog has no one main action for Enter on its empty space.
                main_action: None,
                // SPEC.md's list of dialogs that keep focus on an empty click does not name the trash.
                keeps_focus_on_empty_click: false,
                returns_to,
                // Close and Escape stay available during a request (SPEC.md); an open question goes first, as the old
                // band did ([`TrashDialog::dismissed`]).
                on_dismiss: Some(TrashDialog::dismiss_hook(dialog.downgrade())),
                placement: layers::DialogPlacement::Centered,
                dismiss_on_outside_press: false,
            },
            mark,
            window,
            cx,
        ) else {
            return;
        };
        dialog.update(cx, |dialog, _| dialog.layer = Some(layer));
        self.dialog = Some((layer, dialog.downgrade()));
        self.measure(cx);
    }

    /// Delete checkout `id` on `host` at once, as an entry's Delete or Retry does: frozen from the state drawn now, and
    /// sent only while every guard allows it (the registry read, no other trash delete of this window's, and the
    /// host's own [`HostSnapshot::entry_target`]). A press the guards refuse does nothing; the dialog draws the button
    /// unavailable then.
    pub(crate) fn delete(&mut self, host: i64, id: &str, cx: &mut Context<Self>) {
        let snapshot = self.snapshot(cx);
        if !snapshot.hosts_known || snapshot.delete_in_flight {
            return;
        }
        let Some(target) = snapshot
            .hosts
            .iter()
            .find(|shown| shown.id == host)
            .and_then(|shown| shown.entry_target(id))
        else {
            return;
        };
        self.send(target, cx);
        cx.notify();
    }

    /// Retry the failed cleanup of checkout `id` on `host` from the notices panel, through the same command and guards
    /// as the dialog's Retry ([`Snapshot::retry_target`] says when it goes ahead); a press the guards refuse does
    /// nothing. Its answer is applied as any delete's is ([`TrashButton::deleted`]): a refusal is kept for the dialog
    /// and the panel, and acceptance alone never resolves the notice (the helm resolves it from a later listing).
    pub(crate) fn retry(&mut self, host: i64, id: &str, cx: &mut Context<Self>) {
        if let Some(target) = self.snapshot(cx).retry_target(host, id) {
            self.send(target, cx);
            cx.notify();
        }
    }

    /// Send a confirmed delete's frozen `batches`, one request per host, each host progressing on its own (SPEC.md:
    /// "sends one request per host with its frozen IDs; the hosts progress independently"). The batches are what the
    /// question showed, never a selection made again now. They go out only while the registry is read, no other trash
    /// delete of this window's runs (the trash control prevents overlapping deletes of its own), and every host is
    /// still on the connection its batch was listed on, with its own key free (SPEC.md puts bulk cleanup under the
    /// host-listing and operation guards, and the old dispatch rechecked them when pressed); otherwise nothing is sent,
    /// as the old dispatch refused a frozen selection whose host had reconnected (the reconnect's read then shows the
    /// host's trash afresh). Only batches that went out close the dialog, as the old one stayed open on a refused
    /// answer; a refusal is the reason the dialog shows ([`Refused`]).
    pub(crate) fn delete_batches(
        &mut self,
        batches: Vec<Target>,
        cx: &mut Context<Self>,
    ) -> Result<(), Refused> {
        let snapshot = self.snapshot(cx);
        let shown = |target: &Target| snapshot.hosts.iter().find(|host| host.id == target.host);
        // The old dispatch's order: work in the way first, then whether the frozen hosts are still current.
        if snapshot.delete_in_flight
            || batches
                .iter()
                .any(|target| shown(target).is_some_and(|host| host.busy))
        {
            return Err(Refused::Busy);
        }
        let current = batches.iter().all(|target| {
            shown(target)
                .is_some_and(|host| host.connected && host.incarnation == target.incarnation)
        });
        if !snapshot.hosts_known || !current {
            return Err(Refused::Connection);
        }
        for target in batches {
            self.send(target, cx);
        }
        cx.notify();
        Ok(())
    }

    /// Start `target`'s delete as work on its host's trash key, and apply its answer when it settles
    /// ([`TrashButton::deleted`]).
    ///
    /// A staged run's host is never sent a delete: its listing names no real folder (`agent_startup`, "Staged host
    /// values"), so the delete is refused here, as a refusal the dialog keeps showing for those checkouts.
    fn send(&mut self, target: Target, cx: &mut Context<Self>) {
        #[cfg(any(feature = "agent", test))]
        if crate::agent_startup::staged_trash(target.host, cx).is_some() {
            // A refusal for each checkout, as a host refuses one it will not delete, so the dialog says why beside
            // each entry rather than as an unconfirmed reply.
            let refused: listing::Outcome = Ok(CheckoutTrashDeleted {
                removed: Vec::new(),
                issues: target
                    .ids
                    .iter()
                    .map(|id| farhelm_proto::CheckoutTrashIssue {
                        id: id.clone(),
                        path: String::new(),
                        message: STAGED_DELETE_REFUSAL.to_owned(),
                    })
                    .collect(),
            });
            let kept = self.current.current(cx);
            listing::record_outcome(
                kept.notices.entry(target.host).or_default(),
                &target.ids,
                &refused,
            );
            cx.notify();
            return;
        }
        let (Some(operations), Ok(command)) =
            (session::current_operations(cx), delete_command(&target))
        else {
            return;
        };
        let button = cx.weak_entity();
        let key = OpKey::trash(&HostView::key(target.host));
        // The host's newest read as the delete goes out, so its answer can tell whether a read has gone out since.
        let read_before = self
            .current
            .current(cx)
            .hosts
            .get(&target.host)
            .map_or(0, |entry| entry.serial);
        let started = operations.update(cx, |operations, cx| {
            operations.start(&key, command, cx, move |settled, cx| {
                let _ = button.update(cx, |button, cx| {
                    button.deleted(&target, read_before, settled, cx)
                });
            })
        });
        if let Err(not_started) = started {
            tracing::debug!(?not_started, "trash delete not sent");
        }
    }

    /// A delete reply can mark the listing being shown while a refresh is pending. A successful refresh issued after
    /// the delete supplies the host's removal state. The helm pushes the advanced trash epoch for an accepted
    /// delete before replying, and that read can now answer before the delete. Reuse a read sent since `read_before`
    /// rather than duplicate it. Refusals are kept only for a host the registry still lists.
    ///
    /// A delete the helm answers with a host timeout (504) is not followed by a read at all. DECISION (the other-dialogs
    /// area's coordinator, 2026-11-15): the helm stops waiting on the host after its 20-second host-wait bound but goes
    /// on with the cleanup, holding that host's trash gate. A read on that host can spend the same bound while
    /// waiting for the gate. When the late cleanup is accepted, the helm advances the epoch and
    /// [`TrashButton::sync`] reads the host then.
    fn deleted(
        &mut self,
        target: &Target,
        read_before: u64,
        settled: &Settled,
        cx: &mut Context<Self>,
    ) {
        let timed_out =
            matches!(&settled.outcome, Answer::Refused(refusal) if refusal.status == HOST_TIMEOUT);
        let outcome: listing::Outcome = match &settled.outcome {
            Answer::Done(body) | Answer::Accepted(body) => {
                operations::decode::<CheckoutTrashDeleted>(Ok(body.clone()), DELETE_SHOWN)
            }
            Answer::Refused(refusal) => Err(refusal.words.clone()),
        };
        let listed = session::current_store(cx).is_some_and(|store| {
            store
                .read(cx)
                .collections()
                .get::<HostView>(&HostView::key(target.host))
                .is_some()
        });
        let kept = self.current.current(cx);
        let read_since = kept
            .hosts
            .get(&target.host)
            .is_some_and(|entry| entry.serial != read_before);
        if let (Ok(deleted), Some(entry)) = (&outcome, kept.hosts.get_mut(&target.host))
            && entry.incarnation == target.incarnation
            && (!read_since || entry.loading)
        {
            listing::accepted_cleanup(entry, &deleted.removed);
        }
        if listed {
            listing::record_outcome(
                kept.notices.entry(target.host).or_default(),
                &target.ids,
                &outcome,
            );
        }
        if !read_since && !timed_out {
            self.read(target.host, cx);
        }
    }
}

/// The frozen command for `target`: its IDs, on the connection they were listed on, which the helm checks before it
/// sends anything to the host.
pub(crate) fn delete_command(target: &Target) -> Result<Command, farhelm_helm_client::PathError> {
    Ok(Command::new(
        Work::TrashDelete,
        Method::Post,
        ApiPath::api(&["checkout-trash", "delete"])?,
        Some(json!({
            "host": target.host,
            "expected_incarnation": target.incarnation,
            "ids": target.ids,
        })),
        DELETE_SHOWN,
    ))
}

/// The can's drawing: open-lid when `full`, an outline otherwise; upside down while `emptying`, lifted by `lift`
/// pixels, turned by `turn` degrees more (the archive cue's wiggle), in `ink`. Shared with the dialog's title, which
/// draws it upright and still.
pub(crate) fn can(full: bool, emptying: bool, lift: f32, turn: f32, ink: gpui::Hsla) -> gpui::Svg {
    let icon = if full {
        Icon::TrashFull
    } else {
        Icon::TrashEmpty
    };
    let drawn = svg()
        .path(icon.path())
        .size(px(CAN))
        .flex_none()
        .text_color(ink);
    let upside_down = if emptying { PI } else { 0. };
    if emptying || turn != 0. {
        drawn.with_transformation(
            Transformation::rotate(gpui::radians(upside_down + turn.to_radians()))
                .with_translation(point(px(0.), px(-lift))),
        )
    } else {
        drawn
    }
}

impl Render for TrashButton {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let snapshot = self.snapshot(cx);
        let palette = theme::palette(cx);
        let (full, summary) = snapshot.summary();
        let (count, _) = snapshot.count();
        let emptying = snapshot.emptying();
        // A live handle keeps the timer running while its window is inactive. Under reduced motion, drop the
        // lift and wiggle too so a toggle mid-animation settles the can into its still pose.
        let moving = ticker::is_active(window, cx) && !ticker::reduced_motion(cx);
        let bobs = emptying && moving;
        if bobs != self.bob.is_some() {
            self.bob = bobs.then(|| ticker::AnimationTicks::start(window, cx));
        }
        let lift = self.bob.as_ref().map_or(0., |bob| bob_lift(bob.count(cx)));
        let shown = window.use_keyed_state("checkout-trash-shown", cx, |_, _| ());
        let id = shown.entity_id();
        if self.shown.entity_id() != id {
            // Hidden buttons do not render again to release their animation handles.
            cx.observe_release(&shown, move |button, _, _| {
                if button.shown.entity_id() == id {
                    button.bob = None;
                    button.wiggle = None;
                    button.wiggle_asked = None;
                }
            })
            .detach();
            self.shown = shown.downgrade();
        }
        // Consume the request on this draw. The archive cue belongs to the button it saw and should never play
        // late when motion returns or the user comes back to the window.
        if self.wiggle_asked.take() == Some(self.shown.entity_id()) && moving {
            self.wiggle = Some(ticker::AnimationTicks::start(window, cx));
        }
        let turn = self
            .wiggle
            .as_ref()
            .and_then(|wiggle| cue::wiggle_angle(wiggle.count(cx)))
            .filter(|_| moving);
        if turn.is_none() {
            self.wiggle = None;
        }
        // `.checkout-trash-button`: the quiet ink, the primary one while the can holds something.
        let ink = theme::color(if full { palette.fg_0 } else { palette.fg_2 });
        // `.session-heading .checkout-trash-button`: 24 pixels high, as the heading's other buttons, with 5 pixels
        // inside its edge at either side and 3 between the can and the count, so the count widens it rather than being
        // cut, as a square would cut it. The button's heading padding (2 above and below, none at the sides) leaves
        // this box the 18 pixels inside the edge.
        let content = div()
            .h(px(CONTENT_HEIGHT))
            .px(px(CONTENT_PAD_X))
            .flex()
            .flex_row()
            .items_center()
            .gap(px(3.))
            .text_color(ink)
            .child(can(full, emptying, lift, turn.unwrap_or(0.), ink))
            .children((count > 0).then(|| {
                div()
                    .text_size(px(COUNT_TEXT))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(count.to_string())
            }));
        let focus = self.focus.clone();
        Button::new(
            BUTTON_ID,
            format!("Checkout trash, {summary}"),
            HoverHelp::sentence(snapshot.help()),
            Role::Button,
        )
        .tier(Tier::Neutral)
        .heading()
        .focus(&self.focus)
        .content(content)
        .on_click(cx.listener(move |button, _, window, cx| {
            // Taken in the handler, after the coordinator counted this press.
            let mark = Mark::take(cx);
            button.open(ReturnTo::Control(focus.clone()), &mark, window, cx);
        }))
    }
}
