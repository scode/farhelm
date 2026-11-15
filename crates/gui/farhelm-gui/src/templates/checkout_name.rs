//! What the launcher's Name field says about a launch template's base name in a managed checkout: the "From template ·
//! numbered if taken" pill beside its label, and "(<base> is taken)" under it once the base was taken and the name moved
//! on to a numbered one.
//!
//! SPEC.md, "Launch templates": "In a managed checkout the Name field shows the resolved name and a "From template ·
//! numbered if taken" pill; an advanced name also shows "(<base> is taken)". Typing in Name or accepting `name:` makes it
//! the user's exact name, removes the pill and disables numbering." Ported from the old launcher's
//! `.launch-composer-name-policy` and `.launch-composer-name-taken` (`list/create_form.rs` and `assets/app.css` at
//! reference commit `24bc0580`).
//!
//! Everything is read from the launcher's draft while drawing: whether the name is a template's base
//! ([`LaunchDraft::name_kind`], which typing clears), and whether the numbering moved past it (the checkout's
//! `naming`, which advances only on a preview the helm answered as taken and restarts on a host or repository change).
//! The launcher draws these parts in its name row; the resolved name itself is the field's own text.

use gpui::{
    AnyElement, App, InteractiveElement as _, IntoElement, ParentElement as _, SharedString,
    StatefulInteractiveElement as _, Styled as _, Window, div, px,
};

use farhelm_proto::launcher::TemplateNameKind;

use crate::components::{HoverHelp, Role, with_hover_help};
use crate::host_text::{HostSentence, HostText};
use crate::launcher::draft::LaunchDraft;
use crate::theme::{self, Palette, metrics};

/// `.launch-composer-name-policy`: 10-pixel text in a pill 2 by 6 pixels around it.
const PILL_TEXT: f32 = 10.;
/// `.launch-composer-name-taken`: 11-pixel text.
const TAKEN_TEXT: f32 = 11.;

/// The template base name the draft's managed checkout numbers, and whether the base was taken (the name shown has
/// moved on to a numbered one), or `None` when the Name field shows no template base: a folder destination, a fixed
/// name, a name the user typed, or no name. The old launcher's `template_base`.
pub(crate) fn template_base(draft: &LaunchDraft) -> Option<(String, bool)> {
    let checkout = draft.checkout.as_ref()?;
    let base = draft.name.value();
    (draft.name_kind == Some(TemplateNameKind::Base) && !base.trim().is_empty())
        .then(|| (base.clone(), checkout.naming.is_numbered()))
}

/// The pill beside the Name label, saying the name came from a template and is numbered when its folder is taken.
pub(crate) fn pill(palette: &Palette, window: &mut Window, cx: &mut App) -> AnyElement {
    let id = SharedString::from("launcher-name-policy");
    let help = HoverHelp::tooltip(
        "This template supplies a base name; Farhelm adds a number when its managed-checkout folder is already taken",
    );
    let pill = div()
        .id(id.clone())
        .debug_selector(|| "launcher-name-policy".into())
        .role(Role::Label)
        .aria_label("From template · numbered if taken")
        // At most the label column's width, so its words wrap inside it, as the old pill, a shrinkable item in that
        // column, did; GPUI measures text unwrapped, so without a limit it would spill into the field's column.
        .max_w_full()
        .py(px(2.))
        .px(px(6.))
        .border_1()
        .border_color(theme::color(palette.border_dim))
        .rounded(metrics::RADIUS_LG)
        .bg(theme::color(palette.well))
        .text_size(px(PILL_TEXT))
        .text_color(theme::color(palette.fg_dim))
        .child("From template · numbered if taken");
    with_hover_help(pill, &id, &help, true, window, cx).into_any_element()
}

/// The note under the Name field once `base` was taken: "(<base> is taken)", the base drawn as host text, since a
/// template's name may come from an agent or the command line.
pub(crate) fn taken_note(
    base: &str,
    palette: &Palette,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let id = SharedString::from("launcher-name-taken");
    let help = HoverHelp::tooltip(
        "The template's base checkout name is already taken; this launch uses the first available numbered name",
    );
    let sentence = HostSentence::new("launcher-name-taken-text")
        .text("(")
        .host(&HostText::from_host(base))
        .text(" is taken)");
    let note = div()
        .id(id.clone())
        .debug_selector(|| "launcher-name-taken".into())
        .role(Role::Label)
        .aria_label(sentence.accessible_name())
        .min_w_0()
        .text_size(px(TAKEN_TEXT))
        .text_color(theme::color(palette.fg_dim))
        .child(sentence);
    with_hover_help(note, &id, &help, true, window, cx).into_any_element()
}
