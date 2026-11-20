//! Host status on the rail: what an avatar draws over its host's mark and adds to its name, and the `↑` the Hosts
//! control carries while some avatar shows an update.
//!
//! SPEC.md, "Session list" (per-host connection state on the rail, the Hosts control's `↑`). Ported from the old app's
//! `hosts.rs` (`rail_presentation`, `HostRailAvatar`), `hosts/updates.rs` (`HostUpdates::toggle_label`), the Hosts
//! toggle in `list/view.rs`, and the stylesheet's `.host-rail-state`, `.host-rail-update`, `.host-rail-attention`,
//! `.hosts-toggle-update` and progress-ring rules, at reference commit `24bc0580`.
//!
//! Every fact here is read, never kept: the phase from the pushed host entry, and the update badge, the work going on
//! and the setup or Update warning from [`crate::hosts::status`], which owns those rules so that the rail, the Hosts
//! panel's inline `↑ Update` and the attention list cannot disagree. This module only draws them:
//!
//! - **The connection dot**, at the mark's lower right on a backing in the rail's own background, in the ink of the
//!   phase's tone ([`crate::hosts::tone_ink`], which the Hosts panel's phase dot uses too).
//! - **The update badge**, an amber `↑` at the upper right. Amber for both urgencies: the old sheet kept the rail's
//!   arrows amber and gave urgency's red only to the Hosts control ("urgency is specific to this toggle").
//! - **The ring**, a dashed accent circle while a setup, Update or uninstall runs, in the inverse ink on the pressed
//!   avatar and back in the accent while the Hosts panel shows (when the pressed avatar wears its quieter look).
//! - **The warning mark**, an amber dot at the lower left, while a setup or Update failed unseen.
//! - **Dimming**, the mark at 45% for a host that cannot serve sessions now: unreachable, suspended, or a sandbox
//!   starting, suspending or resuming, as the old rail dimmed them.
//!
//! The marks are pictures. What they say is in the avatar's accessible name and hover help ([`state_words`]), and for
//! the Hosts control in its name and help ([`hosts_name`]), so none of them is a node of its own.
//!
//! A changing sandbox keeps its status dot over the dimmed host icon.

use farhelm_proto::ui_channel::HostView;
use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, FontWeight, InteractiveElement, IntoElement, ParentElement, Styled, div, px,
};

use crate::assets::Icon;
use crate::hosts::identity::phase;
use crate::hosts::offers::runs_newer_version;
use crate::hosts::status::{HostStatus, Urgency};
use crate::hosts::tone_ink;
use crate::theme::{self, Palette, metrics};

/// `.host-rail-avatar[data-host-dimmed="true"] .host-kind-icon`'s opacity.
const DIMMED: f32 = 0.45;

/// `.host-rail-state` and `.host-rail-attention`: the backing's distance from the box's corner and its padding around
/// the 8-pixel dot.
const CORNER_MARK_INSET: f32 = 1.;
const CORNER_MARK_PAD: f32 = 2.;

/// `.host-rail-update` and `.hosts-toggle-update`: the badge's offsets from the box's upper right (`top: -2px; right:
/// 0`) and its side padding.
const BADGE_TOP: f32 = -2.;
const BADGE_PAD_X: f32 = 3.;

/// The progress ring (`.host-rail-icon::after`): its inset from the box's edge and its dashed edge's width.
const RING_INSET: f32 = 1.;

/// `.host-rail-icon`: the avatar's 40-pixel icon box, which holds the mark and anchors every status mark. The old sheet
/// gave it `margin: -1px`, so it covers the button's 1-pixel edge and the marks sit where they did in the old rail.
const ICON_BOX: f32 = 40.;

/// What the avatar adds to its identity, in the old rail's order: the phase's words; then the work going on, or else
/// "update available" when the badge shows; "credentials not delivered" for a sandbox or container whose credentials
/// were not delivered ([`phase::credential_warning`]), and "tunnels failed, retrying" for a connected host whose
/// forwards failed ([`phase::tunnels_failed`]), both of which also turn its dot amber; the too-new explanation for a
/// host running a newer Farhelm; the last-read wording after a failed registry refresh (`last_read`); and the
/// warning's explanation. The rail joins it to the host's identity as "<identity>: <words>".
pub(super) fn state_words(host: &HostView, status: &HostStatus, last_read: bool) -> String {
    let mut words = status
        .sign_in
        .map_or_else(|| phase::words(&host.state), |state| state.words())
        .to_owned();
    match (&status.live, status.update) {
        // One sentence: the verb is capitalized where it stands alone (the attention list's working rows), not here.
        (Some(live), _) => {
            words.push_str("; ");
            words.push_str(&live.activity.verb().to_lowercase());
        }
        (None, Some(_)) => words.push_str("; update available"),
        (None, None) => {}
    }
    // The old rail's words for a machine's credentials or forwards failing beside its connection, part of the phase's
    // words there, so before the explanations.
    if phase::credential_warning(host).is_some() {
        words.push_str("; credentials not delivered");
    }
    if phase::tunnels_failed(host) {
        words.push_str("; tunnels failed, retrying");
    }
    if runs_newer_version(&host.state) {
        words.push_str("; ");
        words.push_str(&phase::hover(&host.state));
    }
    if last_read {
        words.push_str("; last state read, host list refresh failed");
    }
    if status.warned {
        words.push_str("; setup or update needs attention; open Hosts for details");
    }
    words
}

/// Whether the host's mark is dimmed: it cannot serve sessions now. The old `rail_presentation`'s rule, which also
/// covers sandboxes, whose sessions are out of reach while suspended or changing.
pub(super) fn dimmed(host: &HostView) -> bool {
    use farhelm_proto::ui_channel::HostStateView;
    matches!(
        host.state,
        HostStateView::Unreachable { .. }
            | HostStateView::Suspended { .. }
            | HostStateView::TensorlakeChanging { .. }
    )
}

/// The avatar's icon box, for its button's content: the host's `mark` (dimmed when [`dimmed`]) with its status marks
/// over it ([`avatar_marks`]) and `tracker`, the anchor tracker of the host menu the avatar opens. The marks and the
/// menu are anchored to this box, not to the button, as the old sheet anchored them ("Only the inner icon box anchors
/// corner marks and host menus"), so they keep their places when the rail's rows widen to the expanded rail's names.
pub(super) fn icon_box(
    mark: AnyElement,
    host: &HostView,
    status: &HostStatus,
    pressed: bool,
    quiet: bool,
    tracker: AnyElement,
    palette: &Palette,
) -> AnyElement {
    div()
        .relative()
        .flex_none()
        .size(px(ICON_BOX))
        .m(px(-1.))
        .flex()
        .items_center()
        .justify_center()
        .child(if dimmed(host) {
            div().opacity(DIMMED).child(mark).into_any_element()
        } else {
            mark
        })
        .child(avatar_marks(host, status, pressed, quiet, palette))
        .child(tracker)
        .into_any_element()
}

/// The ring's ink: the inverse ink while the avatar is `pressed` in its filled look, else the accent, which is also its
/// ink on a pressed avatar while `quiet` (the Hosts panel shows), when the chosen entry wears its quieter look.
pub(super) fn ring_ink(pressed: bool, quiet: bool, palette: &Palette) -> theme::Token {
    if pressed && !quiet {
        palette.on_fill
    } else {
        palette.accent
    }
}

/// The marks drawn over an avatar's icon box: the ring when work runs, then the dot, the badge and the warning mark
/// over it, as the old sheet lifted the corner marks over the ring so that each stays readable while work runs. The
/// ring's ink is [`ring_ink`]'s.
///
/// Each mark records a debug selector (`host-rail-<mark>-<id>`, recording nothing outside test builds), so tests can
/// tell which marks an avatar draws; their colors and places are the screenshot goldens' to check.
fn avatar_marks(
    host: &HostView,
    status: &HostStatus,
    pressed: bool,
    quiet: bool,
    palette: &Palette,
) -> AnyElement {
    let id = host.id;
    let ring_ink = ring_ink(pressed, quiet, palette);
    div()
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        .when_some(status.live.as_ref(), |marks, _| {
            marks.child(
                div()
                    .debug_selector(|| format!("host-rail-ring-{id}"))
                    .absolute()
                    .top(px(RING_INSET))
                    .left(px(RING_INSET))
                    .right(px(RING_INSET))
                    .bottom(px(RING_INSET))
                    .border_2()
                    .border_dashed()
                    .border_color(theme::color(ring_ink))
                    .rounded_full(),
            )
        })
        .child(
            corner_dot(tone_ink(phase::host_tone(host), palette), palette)
                .debug_selector(|| format!("host-rail-dot-{id}"))
                .right(px(CORNER_MARK_INSET)),
        )
        // The upper left is free even when the host also has an update or an unacknowledged setup failure.
        .when(status.sign_in.is_some(), |marks| {
            marks.child(
                badge_base(palette.warn, palette)
                    .left_0()
                    .p(px(2.))
                    .debug_selector(|| format!("host-rail-sign-in-{id}"))
                    .child(
                        gpui::svg()
                            .path(Icon::Key.path())
                            .size(px(12.))
                            .text_color(theme::color(palette.warn)),
                    ),
            )
        })
        .when(status.update.is_some(), |marks| {
            marks.child(
                badge(palette.warn, palette).debug_selector(|| format!("host-rail-update-{id}")),
            )
        })
        .when(status.warned, |marks| {
            marks.child(
                corner_dot(palette.warn, palette)
                    .debug_selector(|| format!("host-rail-warning-{id}"))
                    .left(px(CORNER_MARK_INSET)),
            )
        })
        .into_any_element()
}

/// A dot in `ink` on a round backing in the rail's background, at the box's bottom; the caller sets its side.
fn corner_dot(ink: theme::Token, palette: &Palette) -> gpui::Div {
    div()
        .absolute()
        .bottom(px(CORNER_MARK_INSET))
        .p(px(CORNER_MARK_PAD))
        .rounded_full()
        .bg(theme::color(palette.bg_1))
        .child(
            div()
                .size(metrics::STATUS_DOT)
                .rounded_full()
                .bg(theme::color(ink)),
        )
}

/// The `↑` badge at the box's upper right, in `ink` on the raised background.
fn badge(ink: theme::Token, palette: &Palette) -> gpui::Div {
    badge_base(ink, palette)
        .right_0()
        .px(px(BADGE_PAD_X))
        .font_weight(FontWeight::SEMIBOLD)
        .child("↑")
}

/// The raised backing shared by badges at either top corner of the avatar.
fn badge_base(ink: theme::Token, palette: &Palette) -> gpui::Div {
    div()
        .absolute()
        .top(px(BADGE_TOP))
        .rounded(metrics::RADIUS_MD)
        .bg(theme::color(palette.bg_2))
        .text_color(theme::color(ink))
}

// ---- The Hosts control -----------------------------------------------------------------------------------------------

/// The rail's update badges counted for the Hosts control: how many avatars show one, and whether any of them is
/// required.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct Updates {
    pub(super) count: usize,
    pub(super) required: bool,
}

impl Updates {
    /// Count one avatar's badge, if it shows one.
    pub(super) fn add(&mut self, update: Option<Urgency>) {
        if let Some(urgency) = update {
            self.count += 1;
            self.required |= urgency == Urgency::Required;
        }
    }
}

/// The Hosts control's accessible name: the old `toggle_label`, "Hosts" while no avatar has an update, else how many
/// hosts have one.
pub(super) fn hosts_name(updates: Updates) -> String {
    match updates.count {
        0 => "Hosts".to_owned(),
        1 => "Hosts: 1 host has an update".to_owned(),
        count => format!("Hosts: {count} hosts have updates"),
    }
}

/// The Hosts control's hover help: its name with what it does, "Hosts: show host management" while no avatar has an
/// update.
pub(super) fn hosts_help(updates: Updates) -> String {
    match updates.count {
        0 => "Hosts: show host management".to_owned(),
        _ => format!("{}; show host management", hosts_name(updates)),
    }
}

/// The Hosts control's `↑` ink: red when any update is required, amber otherwise.
pub(super) fn hosts_badge_ink(updates: Updates, palette: &Palette) -> theme::Token {
    if updates.required {
        palette.danger_strong
    } else {
        palette.warn
    }
}

/// The Hosts control's `↑`, while any avatar has an update, in [`hosts_badge_ink`]. Unlike an avatar's marks it is
/// anchored to the button itself ([`crate::components::Button::badge`]), as the old toggle's was.
pub(super) fn hosts_badge(updates: Updates, palette: &Palette) -> Option<AnyElement> {
    (updates.count > 0).then(|| {
        badge(hosts_badge_ink(updates, palette), palette)
            .debug_selector(|| "hosts-panel-toggle-update".into())
            .into_any_element()
    })
}
