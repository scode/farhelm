//! The Delete notice band: the newest notice a completed Delete carried, shown across the list's column under the status
//! segments until the user dismisses it or a newer notice replaces it.
//!
//! SPEC.md, "Managed checkouts": when a deleted session's checkout could not be archived, Delete still removes the
//! session, and "That outcome must never be silent: Delete completes with a visible notice that the checkout was not
//! archived, naming the path left behind and, where known, why. Wherever Delete's result is shown, the notice is shown
//! with it." The helm's Delete reply carries that notice as its `notice`, and a Replace's reply carries its source's as
//! `delete_notice`. Ported from the old app's `DeleteNotice` and its `.delete-notice` band in `list/view.rs`, and the
//! stylesheet's `.delete-notice` rules, at reference commit `24bc0580`. Here's the TLDR of the rules:
//!
//! - **Every Delete this window sends reports here.** The row's, the header's, the Delete shortcut's and the bulk
//!   Delete's all go through [`super::delete::start`], whose completion hands the reply's notice to [`publish`]; a plain
//!   Replace and the launcher's Replace with settle through [`super::replace::settle`], which hands over the source's.
//!   A Delete that an agent or the command line sent shows nothing here, since its reply never reaches this window.
//! - **A notice replaces the band; nothing to report leaves it.** A newer notice takes the band's place whatever it
//!   showed, and a Delete whose reply has no notice (an archived checkout, or no checkout at all) leaves an earlier
//!   notice where it is, as the old `publish(None)` did. Dismiss is the only other way the band goes.
//! - **The helm's words, unprefixed.** The notice is host text (it names a path on the session's host) and is drawn as
//!   such ([`HostSentence`]), with nothing in front of it: the band says which operation it is about by being the
//!   Delete band, where a row's outcome line names its operation.
//! - **Per UI session.** The band is kept in a [`PerSession`], so a reset starts the window with no band, as the old
//!   app's reload did, and switching the open session leaves it alone. The list redraws when it changes ([`observe`]).
//! - **A store of its own, not the notice center's or the operation store's.** The migration's orchestrator prescribed
//!   this shape on 2026-11-16, when it asked for the band, and its review gate asked for the reasons to be recorded.
//!   The notice center ([`crate::notices`]) carries `Copy` notices in the app's own words and keeps them across a reset
//!   (the connection-lost notice is posted because of one), while this notice is host text that must go with the UI
//!   session, so carrying it there would change that foundation beyond its one extension point, a new variant. The
//!   operation store's per-session slot (`super::outcome`) is cleared by the session's next operation and goes with
//!   its row, and the notice exists to outlive the row: showing a Replace's notice on the replacement's row was the
//!   stopgap this replaces, and it had to drop the notice whenever the replacement already had work running. And the
//!   list's own per-session state is out of reach of a Delete's completion, which runs with the app alone, not the
//!   list (the same reason [`super::delete`]'s record of this window's own Deletes is a global).
//! - **Dismiss and focus.** Dismiss takes the band away. A Dismiss that held focus goes with it, and the focus
//!   coordinator then sends focus home (SPEC.md, "Where focus rests": "whenever nothing holds focus it goes to the
//!   selected session's terminal", "the last notice was dismissed" among the causes). Nothing here moves focus itself.
//! - **Rows move, menus stay.** The band moves the rows below it as it comes and goes. An open row menu stays open and
//!   follows its button, as every anchored layer does (the layer manager's "An overlay closes when its subject is gone,
//!   not when it moves"); the old app closed it instead, which the foundations' parity file records as superseded by
//!   SPEC.md's "Things changing underneath".
//!
//! While the Hosts panel shows in place of the list's column the band is not drawn, as the old band was hidden with the
//! rest of the column, and it shows again when Sessions comes back.
//!
//! NOTE: A bulk Delete of several sessions whose checkouts could not be archived shows only the last notice to arrive:
//! each replaces the one before, as in the old app, whose per-row completion published each reply in turn. SPEC.md
//! asks that each such outcome be visible, so this is a known gap; the sandbox and container Delete, which reworks the
//! band's text anyway, decides whether one batch's notices are joined.
//!
//! NOTE: The sandbox and container Delete adds the rest of the old band: its "The sandbox stays: N other sessions are
//! in it" result shown ahead of the reply's notice, the tag naming the environment that count describes, and the count's
//! retirement by a later Delete on the same environment or by a hosts read that no longer lists it (the old
//! `KeptNotice`, `environment_band` and `kept_band_after_hosts_read`). Nothing here sends that Delete yet, so none of
//! it is ported until it has a caller.

use gpui::{
    AnyElement, App, BorrowAppContext as _, Context, Global, InteractiveElement, IntoElement,
    ParentElement, Role, Styled, Subscription, div, px,
};

use super::SessionList;
use crate::components::{Button, HoverHelp};
use crate::host_text::{HostSentence, HostText};
use crate::session::PerSession;
use crate::theme::metrics;

/// The band's element id and debug selector: tests find the band, and its place in the column, by this.
pub(super) const BAND_ID: &str = "session-delete-notice";

/// The notice's element id: the host sentence that carries the helm's words, named by them.
pub(super) const TEXT_ID: &str = "session-delete-notice-text";

/// The Dismiss button's element id.
pub(super) const DISMISS_ID: &str = "session-delete-notice-dismiss";

/// `.delete-notice`'s gap between the notice and Dismiss: `0.5rem`.
const GAP: f32 = 8.;

/// The notice the band shows under the current UI session, or `None` while it shows nothing. App-wide, like the
/// selection, which assumes one window (see the module docs for why it is not the list's own state).
#[derive(Default)]
struct Band(PerSession<Option<String>>);

impl Global for Band {}

/// A Delete this window sent finished with `notice`: show it in place of whatever the band showed. `None` leaves the
/// band as it is, so a later Delete with nothing to report never takes an earlier notice away. Nothing here judges the
/// words; the helm sends a notice only when it has something to say.
pub(super) fn publish(notice: Option<String>, cx: &mut App) {
    let Some(notice) = notice else {
        return;
    };
    cx.update_default_global::<Band, _>(|band, cx| *band.0.current(cx) = Some(notice));
}

/// The user dismissed the band: show nothing until the next notice.
fn dismiss(cx: &mut App) {
    cx.update_default_global::<Band, _>(|band, cx| *band.0.current(cx) = None);
}

/// The notice the band shows now, read without starting anything over: `None` with no notice, after a reset until the
/// next [`publish`], and while the window is inert.
fn shown(cx: &App) -> Option<String> {
    cx.try_global::<Band>()
        .and_then(|band| band.0.read(cx))
        .and_then(Clone::clone)
}

/// Redraw `list` whenever the band changes. A Delete's completion runs inside the operation store's update, which the
/// list redraws for anyway, but Dismiss changes nothing else the list reads, so the list watches the band itself rather
/// than rely on whatever else changed beside it.
pub(super) fn observe(cx: &mut Context<SessionList>) -> Subscription {
    cx.observe_global::<Band>(|_, cx| cx.notify())
}

/// The band for the list's column, or `None` while there is no notice. The old `.delete-notice` row: the notice at the
/// old `.status` line's 14 pixels with its 28-pixel padding, taking the width and wrapping anywhere in a long path,
/// then Dismiss (the old ghost `.btn`, "Dismiss: hide this notice") at the band's top edge.
pub(super) fn band(cx: &App) -> Option<AnyElement> {
    let notice = shown(cx)?;
    let text = HostSentence::new(TEXT_ID).host(&HostText::from_host(notice));
    let dismiss = Button::new(
        DISMISS_ID,
        "Dismiss",
        HoverHelp::tooltip("Dismiss: hide this notice"),
        Role::Button,
    )
    .on_click(|_, _, cx| dismiss(cx));
    Some(
        div()
            .id(BAND_ID)
            .debug_selector(|| BAND_ID.into())
            .flex()
            .flex_row()
            .items_start()
            .gap(px(GAP))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .p(metrics::LIST_STATUS_PAD)
                    .text_size(metrics::TEXT_ROW_TITLE)
                    .child(text),
            )
            .child(div().flex_none().child(dismiss))
            .into_any_element(),
    )
}
