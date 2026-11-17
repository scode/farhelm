//! How the launcher's text fields show a value holding characters that change how text reads (a direction override, a
//! zero-width space; [`farhelm_proto::text::is_presentation_unsafe`]): with each such character spelled `<U+XXXX>`. The
//! wrapper prefix, initial prompt, Custom flags, command line and resume command go through here; the closed model
//! field's display does the same (`model_field`).
//!
//! Why: these values often come from somewhere other than the user's typing (a session's stored launch, which may
//! have come from another host or an agent, a remembered prefix, a template), and a field showing them raw could hide or
//! reverse what the words say. The draft always holds the raw text, so an untouched value is sent byte for byte; only
//! the field shows the spelling, set by the launcher ([`super::Launcher`]'s `sync_fields`), which raises no edit, so the
//! spelling never flows back into the draft by itself. Ported from the old launcher at reference commit `24bc0580`,
//! field by field, in two kinds:
//!
//! - **Read-only, with a Clear button**, where editing the spelling would turn the escape text into new launch words:
//!   the wrapper prefix in any mode, judged on whatever the field holds, typed text included (`launch_controls.rs`'s
//!   `wrapper_unsafe`); the initial prompt, judged without its newlines and tabs, whose Clear is the prompt's own Clear
//!   initial prompt (`prompt_read_only`); and Restart with's stored command line and resume command while they are
//!   still the stored ones (`list/create_form.rs`'s `restart_command_readonly` and `restart_resume_readonly`).
//! - **Editable**, where an edit takes the field's text as it then reads, the spelling included, as the old fields did:
//!   Custom flags, judged on whatever the field holds (`editable_peer`), and a command line or resume command only while
//!   it is still the one the launcher opened with, such as the ones Clone and Replace with copy from their source
//!   (`create_form/draft.rs`'s `seed_invocation` and `seed_command_fields`, which spelled out only the copied value). A
//!   command the user typed or pasted is shown as typed: it reaches Bash as written (SPEC.md, "Command launch"), and
//!   the `<` and `>` of a spelling would be redirections there.
//!
//! An editable spelling escapes only the hidden characters, as `editable_peer` did, so an edit never turns a literal
//! `<U+` the user typed into an escape of its own; a read-only one uses the app's full host-text spelling
//! ([`farhelm_proto::text::escaped`]), which also marks a literal `<U+`, since it is never edited.
//!
//! Old specs: `restart-with.spec.ts`'s "restart with shows an unsafe stored wrapper prefix without editing its escape
//! text" and "restart with preserves raw command bytes", and `launcher-tabs.spec.ts`'s "agent switch loads that agent's
//! remembered wrapper prefix".

use farhelm_proto::text::{escaped, is_presentation_unsafe, notation};
use gpui::{AnyElement, Context, IntoElement, ParentElement as _, Styled as _, Window, div, px};

use super::{LABEL_COLUMN, LABEL_GAP, Launcher, on_launcher};
use crate::components::{Button, HoverHelp, Role, Tier};
use crate::launcher::draft::{LaunchDraft, Mode};

/// How a field shows its value: the text (spelled out while the value holds a hidden character, else the value as it
/// is), and whether the field is read-only.
pub(super) struct FieldText {
    pub(super) text: String,
    pub(super) read_only: bool,
}

/// The command line and resume command the launcher opened with: what a Clone or Replace with copied from its source,
/// or a Restart with's stored launch. Only these are spelled out while editable; text typed since is the user's own,
/// and so is a template's command, which is applied after the launcher is made and so is shown as typed, as the old
/// launcher showed it.
#[derive(Default)]
pub(super) struct Opened {
    command: String,
    resume: String,
}

impl Opened {
    /// What `draft` holds as the launcher opens.
    pub(super) fn of(draft: &LaunchDraft) -> Self {
        Opened {
            command: draft.command.command.clone(),
            resume: draft.command.resume.clone(),
        }
    }
}

/// Whether `text` holds a hidden character; with `multiline`, a newline or tab does not count, as a message legitimately
/// has them.
fn needs_escaping(text: &str, multiline: bool) -> bool {
    text.chars()
        .any(|ch| !(multiline && matches!(ch, '\n' | '\t')) && is_presentation_unsafe(ch))
}

/// `raw` with only its hidden characters spelled out (the old `editable_peer`).
fn spelled(raw: &str) -> String {
    raw.chars()
        .map(|ch| {
            if is_presentation_unsafe(ch) {
                notation(ch)
            } else {
                ch.to_string()
            }
        })
        .collect()
}

/// `raw` as a field shows it: spelled out while it holds a hidden character (read-only with the full spelling when
/// `lock`, editable with [`spelled`] otherwise), else as it is.
fn shown(raw: &str, multiline: bool, lock: bool) -> FieldText {
    if !needs_escaping(raw, multiline) {
        return FieldText {
            text: raw.to_owned(),
            read_only: false,
        };
    }
    FieldText {
        text: if lock { escaped(raw) } else { spelled(raw) },
        read_only: lock,
    }
}

/// `raw` shown as typed: neither spelled out nor read-only.
fn as_typed(raw: &str) -> FieldText {
    FieldText {
        text: raw.to_owned(),
        read_only: false,
    }
}

/// A value shown but never edited in place (the closed model field): spelled out in full while it holds a hidden
/// character.
pub(super) fn display(raw: &str) -> String {
    if needs_escaping(raw, false) {
        escaped(raw)
    } else {
        raw.to_owned()
    }
}

/// The wrapper prefix, in any mode: the old launcher judged the prefix the field holds, so a remembered prefix loaded
/// by an agent switch is shown the same way as a stored one.
pub(super) fn wrapper(draft: &LaunchDraft) -> FieldText {
    shown(&draft.agent.wrapper, false, true)
}

/// The initial prompt: read-only while it holds a hidden character other than a newline or a tab; shown then with
/// those escaped too, as the old read-only display did.
pub(super) fn prompt(draft: &LaunchDraft) -> FieldText {
    shown(&draft.prompt, true, true)
}

/// Custom flags, spelled out and editable, judged on whatever the field holds.
pub(super) fn flags(draft: &LaunchDraft) -> FieldText {
    shown(&draft.agent.permission_flags, false, false)
}

/// The command line: spelled out while it is still the one the launcher opened with (read-only in Restart with, whose
/// opening value is the session's stored launch; editable otherwise), else as typed.
pub(super) fn command(draft: &LaunchDraft, opened: &Opened) -> FieldText {
    as_opened(&draft.command.command, &opened.command, &draft.mode)
}

/// The resume command, under [`command`]'s rule.
pub(super) fn resume(draft: &LaunchDraft, opened: &Opened) -> FieldText {
    as_opened(&draft.command.resume, &opened.resume, &draft.mode)
}

/// `value` against what its field `opened` with: [`command`]'s rule.
fn as_opened(value: &str, opened: &str, mode: &Mode) -> FieldText {
    if value != opened {
        return as_typed(value);
    }
    shown(value, false, matches!(mode, Mode::RestartWith(_)))
}

/// A Clear button under a field shown read-only, indented to the fields' column: `id`, its words, and its hover help.
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
