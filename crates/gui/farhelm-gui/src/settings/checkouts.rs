//! Settings → Checkouts: the explicitly saved default checkout folder and post-clone command (each a
//! [`super::CheckoutField`] of its own), and the Keep a checkout ready for faster launches switch.
//!
//! SPEC.md, "Managed checkouts" and "Session list" (Checkouts). Ported from the old Settings dialog's Checkouts section
//! and its `CheckoutSpareSettings` (`checkout_spare.rs`) at reference commit `24bc0580`, with its words, its folder
//! note and its documentation link. The two fields are views of their own ([`super::checkout_field`]); this module
//! holds the section's words and the switch.
//!
//! The switch is helm configuration behind `/api/checkout-spare`, which the helm does not push, so it is a
//! [`Configured`] switch like Agent trust's, sharing their state, read, save and failure line ([`super::configured`]):
//! the section reads it once, when it is first shown in an opening, and the switch is unavailable rather than shown off
//! until that read answers ("The read must succeed before the switch becomes editable", the old component's rule). A
//! flip saves at once through the operation store under [`crate::operations::OpKey::preferences`], shared with every
//! Settings save for the reason [`super::agent_trust`] gives. While it is in flight the switch shows the value being
//! saved; a done save becomes the value the section holds, and a refusal shows the value read before, with the helm's
//! reason under the switch. Nothing is retried or read back.

use farhelm_helm_client::{ApiPath, PathError};
use gpui::{App, Div, ParentElement as _, Role, Styled as _, Window, div, px};

use super::Refused;
use super::choices::{self, GROUP_GAP};
use super::configured::{self, Configured};
use crate::components::{Button, Checkbox, HoverHelp, Tier};
use crate::host_text::HostText;
use crate::operations::Command;

/// The default folder field's note, the old Settings dialog's.
pub(super) const FOLDER_NOTE: &str = "Used by hosts without their own folder, which you set in a host's settings. \
                                      ~ means each host's home folder. A missing folder is created on first use. \
                                      Changes affect new checkouts only. Save an empty field to clear the default.";

/// The switch's element id, which tests and the test bridge find it by.
pub(super) const SPARE_ID: &str = "settings-checkout-spare";
pub(super) const SPARE_LABEL: &str = "Keep a checkout ready for faster launches";
pub(super) const SPARE_TOOLTIP: &str = "Prepare one extra checkout per repository and host so the next launch can \
                                       update it instead of cloning again";
/// Its failure line's.
pub(super) const SPARE_ERROR_ID: &str = "settings-checkout-spare-error";
/// The documentation link's.
pub(super) const SPARE_DOCS_ID: &str = "settings-checkout-spare-docs";

/// The page the old help linked to for how a prepared checkout differs from a fresh clone.
const SPARE_DOCS: &str = "https://farhelm.io/docs/using/start-a-session/#prepared-checkouts";

/// The switch's help, the old wording (SPEC.md: it "explains one extra checkout's disk space per repository and host,
/// promptless login-shell preparation, earlier file times and Git hooks/filters, and discard on the next create or
/// after 30 days unused; it links the full earlier-clone caveats").
pub(super) const SPARE_HELP: &str = "On by default. Keeps one extra checkout per repository and host, using extra disk space. \
                          Farhelm prepares it through your login shell without prompting for credentials. The next \
                          launch updates that earlier clone: unchanged files keep older modification times, and Git \
                          hooks and checkout filters run ahead of time, then again only for the update. Turning this \
                          off affects future creates; each repository's spare is discarded at its next create on that \
                          host, or after 30 days unused.";

fn spare_path() -> Result<ApiPath, PathError> {
    ApiPath::api(&["checkout-spare"])
}

/// `PUT /api/checkout-spare` with `{enabled}`.
pub(super) fn spare_command(enabled: bool) -> Result<Command, PathError> {
    configured::enabled_command(spare_path(), enabled)
}

/// Read the switch's choice ([`configured::read_enabled`]).
pub(super) fn read_spare(
    cx: &mut App,
    on_read: impl FnOnce(Result<bool, HostText>, &mut App) + 'static,
) {
    configured::read_enabled(spare_path(), cx, on_read);
}

/// The switch with its help, the documentation link, its failure line and the line saying why a press sent nothing
/// (`refused`, [`choices::busy_line`]). `on_toggle` is what a toggle does.
///
/// The docs link has its own line so it can be focused independently of the switch and its summary.
pub(super) fn spare(
    spare: &Configured,
    focus: &gpui::FocusHandle,
    refused: &Refused,
    on_toggle: impl Fn(bool, &mut Window, &mut App) + 'static,
    cx: &App,
) -> Div {
    let switch = Checkbox::new(
        SPARE_ID,
        SPARE_LABEL,
        HoverHelp::tooltip(SPARE_TOOLTIP),
        Role::Switch,
    )
    .switch()
    .content(super::card::switch_copy(SPARE_LABEL, SPARE_HELP, cx))
    .focus(focus)
    .checked(spare.shown())
    .disabled(!spare.available())
    .on_toggle(on_toggle);
    let link = Button::new(
        SPARE_DOCS_ID,
        "How prepared checkouts differ from a fresh clone ↗",
        HoverHelp::tooltip("Read how prepared checkouts differ from a fresh clone"),
        Role::Link,
    )
    .tier(Tier::Link)
    .on_click(|_, _, cx| cx.open_url(SPARE_DOCS));
    let failure = configured::failure(
        SPARE_ERROR_ID,
        "Could not save or read prepared checkout settings: ".to_owned(),
        spare,
        cx,
    );
    div()
        .flex()
        .flex_col()
        .gap(px(GROUP_GAP))
        .min_w_0()
        .child(switch)
        // In a row of its own, so the link is as wide as its words rather than stretched across the column.
        .child(div().flex().child(link))
        .children(failure)
        .children(choices::busy_line(refused, SPARE_ID, cx))
}
