//! Stored text shown escaped: a wrapper prefix, command line or resume command holding characters that change how text
//! reads (a direction override, a zero-width space; [`farhelm_proto::text::is_presentation_unsafe`]) is shown with
//! each such character spelled `<U+XXXX>`, read-only, beside a Clear button that empties it and makes the field
//! editable again.
//!
//! Why: these values come from somewhere other than the user's typing (a session's stored launch, which may have come
//! from another host or an agent, or a remembered prefix), and a field showing them raw could hide or reverse what the
//! words say. Editing the escaped spelling is not allowed either, because the escape text would then become new launch
//! words; so an untouched value is sent byte for byte as stored, and the only way to change it is to clear it and type
//! a new one. Ported from the old launcher at reference commit `24bc0580`: `launch_controls.rs`'s `wrapper_unsafe`
//! (any mode, judged on the prefix the field holds) and `list/create_form.rs`'s `restart_command_readonly` and
//! `restart_resume_readonly` (Restart with only, while the value is still the stored one), with its old specs
//! `restart-with.spec.ts`'s "restart with shows an unsafe stored wrapper prefix without editing its escape text" and
//! "restart with preserves raw command bytes", and `launcher-tabs.spec.ts`'s "agent switch loads that agent's
//! remembered wrapper prefix".
//!
//! The draft always holds the raw text; only the field shows the escaped spelling, set by the launcher
//! ([`super::Launcher`]'s `sync_fields`), which raises no edit, so the spelling never flows back into the draft.

use farhelm_proto::text::{escaped, is_presentation_unsafe};
use gpui::{AnyElement, Context, IntoElement, ParentElement as _, Styled as _, Window, div, px};

use super::{LABEL_COLUMN, LABEL_GAP, Launcher, on_launcher};
use crate::components::{Button, HoverHelp, Role, Tier};
use crate::launcher::draft::{LaunchDraft, Mode, RestartBaseline};

/// `raw` as its field shows it while it holds a presentation-unsafe character; `None` when it reads as written.
fn escaped_if_unsafe(raw: &str) -> Option<String> {
    raw.chars()
        .any(is_presentation_unsafe)
        .then(|| escaped(raw))
}

/// The wrapper prefix's escaped spelling, in any mode: the old launcher judged the prefix the field holds, so a
/// remembered prefix loaded by an agent switch is shown the same way as a stored one.
pub(super) fn wrapper(draft: &LaunchDraft) -> Option<String> {
    escaped_if_unsafe(&draft.agent.wrapper)
}

/// The command line's escaped spelling, in Restart with while it is still the stored command.
pub(super) fn command(draft: &LaunchDraft) -> Option<String> {
    let stored = stored_command(draft)?;
    if draft.command.command != stored.command {
        return None;
    }
    escaped_if_unsafe(&draft.command.command)
}

/// The resume command's escaped spelling, in Restart with while it is still the stored one.
pub(super) fn resume(draft: &LaunchDraft) -> Option<String> {
    let stored = stored_command(draft)?;
    if Some(draft.command.resume.as_str()) != stored.resume.as_deref() {
        return None;
    }
    escaped_if_unsafe(&draft.command.resume)
}

/// The stored command launch Restart with edits; `None` outside Restart with or for an agent launch.
fn stored_command(draft: &LaunchDraft) -> Option<&farhelm_proto::CommandLaunch> {
    match &draft.mode {
        Mode::RestartWith(restart) => match &restart.baseline {
            RestartBaseline::Command(stored) => Some(stored),
            RestartBaseline::Agent(_) => None,
        },
        Mode::New | Mode::Clone | Mode::ReplaceWith { .. } => None,
    }
}

/// A Clear button under a field shown escaped, indented to the fields' column: `id`, its words, and its hover help.
pub(super) fn clear_button(
    id: &'static str,
    words: &'static str,
    help: &'static str,
    act: fn(&mut Launcher, &mut Window, &mut Context<Launcher>),
    cx: &mut Context<Launcher>,
) -> AnyElement {
    div()
        .pl(px(LABEL_COLUMN + LABEL_GAP))
        .child(
            Button::new(id, words, HoverHelp::tooltip(help), Role::Button)
                .tier(Tier::Neutral)
                .on_click(on_launcher(cx.weak_entity(), act)),
        )
        .into_any_element()
}

impl Launcher {
    /// Clear wrapper prefix: the user's own (empty) choice from now on, editable.
    pub(super) fn clear_wrapper(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.draft.edit_wrapper(String::new());
        self.sync_fields(window, cx);
        self.changed(cx);
    }

    /// Clear agent command: Restart with's command emptied and editable.
    pub(super) fn clear_command(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.draft.edit_command(String::new());
        self.sync_fields(window, cx);
        self.changed(cx);
    }

    /// Clear resume command: Restart with's resume command emptied and editable.
    pub(super) fn clear_resume(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.draft.edit_command_resume(String::new());
        self.sync_fields(window, cx);
        self.changed(cx);
    }
}
