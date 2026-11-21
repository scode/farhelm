//! How the launcher's text fields show a value holding characters that change how text reads (a direction override, a
//! zero-width space; [`farhelm_proto::text::is_presentation_unsafe`]): with each such character spelled `<U+XXXX>`. The
//! wrapper prefix, initial prompt, Custom flags, command line, resume command and Clone's copied folder and name go
//! through here. The closed model field's display does the same (`model_field`).
//!
//! Why: these values often come from somewhere other than the user's typing (a session's stored launch, which may
//! have come from another host or an agent, a remembered prefix, a template), and a field showing them raw could hide or
//! reverse what the words say. The draft always holds the raw text, so an untouched value is sent byte for byte; only
//! the field shows the spelling, set by the launcher (`sync_fields`), which raises no edit. Ported from the old launcher
//! at reference commit `24bc0580`. One rule per field (`Input::spelling`) decides both how it shows its value and how an
//! edit is read back, so no field can decode an edit and then show the result raw. Here's the TLDR of the three kinds:
//!
//! - **Read-only, with a Clear button**, where editing the spelling would turn the escape text into new launch words:
//!   the wrapper prefix in any mode, judged on whatever the field holds, typed text included; the initial prompt,
//!   judged without its newlines and tabs, whose Clear is the prompt's own Clear initial prompt; and Restart with's
//!   stored command line and resume command while they still hold the stored value. Clear ends that for the rest of
//!   the opening, so retyping the stored command leaves the field editable, as the old field stayed after its first
//!   edit.
//! - **Editable and spelled out** while the value holds a hidden character: Custom flags in every mode, the command line
//!   and resume command (Restart with's once cleared), and Clone's folder and session name, a managed Clone's numbered
//!   name included. An edit turns each complete spelling back into its character before the draft sees it, so typing
//!   beside a spelling never turns it into literal text (in a command, its `<` and `>` would be Bash redirections). A
//!   spelling the user types by hand stands for the same character, which the maintainer accepted on 2026-11-18; so a
//!   copied value holding the literal text `<U+200B>` becomes that character once its field is edited.
//! - **Raw**, shown and taken as typed: the conversation id, and the folder and name outside Clone.
//!
//! An editable spelling escapes only the hidden characters, and the decoder accepts only exactly the spelling the app
//! writes, so `<U+0041>`, `<U+200b>` or `<U+0200B>` stay as typed. A read-only one uses the app's full host-text
//! spelling ([`farhelm_proto::text::escaped`]), which also marks a literal `<U+`, since it is never edited.
//!
//! Old specs: `restart-with.spec.ts`'s "restart with shows an unsafe stored wrapper prefix without editing its escape
//! text" and "restart with preserves raw command bytes", and `launcher-tabs.spec.ts`'s "agent switch loads that agent's
//! remembered wrapper prefix".

use farhelm_proto::text::{escaped, is_presentation_unsafe, notation};
use gpui::{AnyElement, Context, IntoElement, ParentElement as _, Window, div};

use super::{Input, Launcher, on_launcher};
use crate::components::{Button, HoverHelp, Role, Tier};
use crate::launcher::draft::{LaunchDraft, Mode};

/// How a field shows its value: the text (spelled out while the value holds a hidden character, else the value as it
/// is), and whether the field is read-only.
pub(super) struct FieldText {
    pub(super) text: String,
    pub(super) read_only: bool,
}

/// Restart with locks an unsafe opening command while its field holds that value. Clear releases the lock for the
/// rest of this opening, so a command retyped after Clear stays editable.
#[derive(Default)]
pub(super) struct Opened {
    command: Option<String>,
    resume: Option<String>,
}

impl Opened {
    /// Only Restart with can lock what was in its opening command fields.
    pub(super) fn of(draft: &LaunchDraft) -> Self {
        if matches!(draft.mode, Mode::RestartWith(_)) {
            return Self {
                command: needs_escaping(&draft.command.command, false)
                    .then(|| draft.command.command.clone()),
                resume: needs_escaping(&draft.command.resume, false)
                    .then(|| draft.command.resume.clone()),
            };
        }
        Self::default()
    }
}

/// Whether `text` holds a hidden character; with `multiline`, a newline or tab does not count, as a message legitimately
/// has them.
fn needs_escaping(text: &str, multiline: bool) -> bool {
    text.chars()
        .any(|ch| !(multiline && matches!(ch, '\n' | '\t')) && is_presentation_unsafe(ch))
}

/// The editable spelling leaves ordinary punctuation alone so the field can still be edited as written.
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

/// Decode the app's exact spelling. Comparing with `notation` keeps case and padding in step with the renderer.
fn unspell(mut visible: &str) -> String {
    let mut raw = String::with_capacity(visible.len());
    while let Some(start) = visible.find("<U+") {
        raw.push_str(&visible[..start]);
        visible = &visible[start..];
        let candidate = visible.as_bytes().iter().take(10).position(|b| *b == b'>');
        if let Some(end @ 7..=9) = candidate
            && let Ok(code) = u32::from_str_radix(&visible[3..end], 16)
            && let Some(ch) = char::from_u32(code)
            && is_presentation_unsafe(ch)
            && notation(ch) == visible[..=end]
        {
            raw.push(ch);
            visible = &visible[end + 1..];
        } else {
            // An invalid prefix can precede a valid spelling. Resume at the end of the prefix.
            raw.push_str("<U+");
            visible = &visible[3..];
        }
    }
    raw.push_str(visible);
    raw
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

/// A field's spelling contract in the current draft. Editable spellings must decode through the same policy that
/// put them on screen. Locked spellings are the full host-text form, with no edit path through that text.
enum Spelling {
    Raw,
    Editable,
    Lock { multiline: bool },
}

impl Input {
    /// Keep the display, edit decoding and resync on the same field policy.
    fn spelling(self, draft: &LaunchDraft, opened: &Opened) -> Spelling {
        match self {
            Self::Folder | Self::Name if matches!(draft.mode, Mode::Clone) => Spelling::Editable,
            Self::Flags => Spelling::Editable,
            Self::Command if opened.command.as_deref() == Some(&draft.command.command) => {
                Spelling::Lock { multiline: false }
            }
            Self::Resume if opened.resume.as_deref() == Some(&draft.command.resume) => {
                Spelling::Lock { multiline: false }
            }
            Self::Command | Self::Resume => Spelling::Editable,
            Self::Wrapper => Spelling::Lock { multiline: false },
            Self::Prompt => Spelling::Lock { multiline: true },
            Self::Folder | Self::Name | Self::Conversation => Spelling::Raw,
        }
    }

    /// Render the effective draft value. A managed Clone uses the numbered checkout title until the user edits it.
    pub(super) fn present(self, draft: &LaunchDraft, opened: &Opened) -> FieldText {
        let name;
        let raw = match self {
            Self::Folder => draft.folder.value(),
            Self::Name => {
                name = draft.shown_name();
                &name
            }
            Self::Conversation => &draft.conversation,
            Self::Wrapper => &draft.agent.wrapper,
            Self::Prompt => &draft.prompt,
            Self::Flags => &draft.agent.permission_flags,
            Self::Command => &draft.command.command,
            Self::Resume => &draft.command.resume,
        };
        match self.spelling(draft, opened) {
            Spelling::Raw => as_typed(raw),
            Spelling::Editable => shown(raw, false, false),
            Spelling::Lock { multiline } => shown(raw, multiline, true),
        }
    }

    /// Interpret a user edit without changing raw fields or reinterpreting values loaded directly into the draft.
    pub(super) fn edited_text(self, text: String, draft: &LaunchDraft, opened: &Opened) -> String {
        if matches!(self.spelling(draft, opened), Spelling::Editable) && text.contains("<U+") {
            unspell(&text)
        } else {
            text
        }
    }

    /// An edit can introduce a hidden character or complete a spelling. Resync any field whose policy can show one.
    pub(super) fn shown_through_text_rules(self, draft: &LaunchDraft, opened: &Opened) -> bool {
        !matches!(self.spelling(draft, opened), Spelling::Raw)
    }
}

/// A Clear button under a read-only value lets the user replace it with ordinary editable text.
pub(super) fn clear_button(
    id: &'static str,
    words: &'static str,
    help: &'static str,
    act: fn(&mut Launcher, &mut Window, &mut Context<Launcher>),
    cx: &mut Context<Launcher>,
) -> AnyElement {
    div()
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
        self.opened.command = None;
        self.draft.edit_command(String::new());
        self.sync_fields(window, cx);
        self.changed(cx);
    }

    /// Clear resume command: Restart with's resume command emptied and editable.
    pub(super) fn clear_resume(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.opened.resume = None;
        self.draft.edit_command_resume(String::new());
        self.sync_fields(window, cx);
        self.changed(cx);
    }
}

#[cfg(test)]
mod tests {
    use super::{spelled, unspell};

    /// Nearby edits must keep the bytes the renderer spelled out, including controls and supplementary characters.
    #[test]
    fn editable_spellings_round_trip() {
        let raw = "é\u{0}\t\n\u{1b}\u{200b}\u{202e}\u{e0001}";
        assert_eq!(unspell(&spelled(raw)), raw);
    }

    /// Accept only spellings the renderer could write. A malformed prefix may sit beside a real spelling.
    #[test]
    fn only_canonical_hidden_characters_decode() {
        let literal = "<U+0041><U+0200B><U+200b><U+D800><U+110000><U+200Bえ><U+200B";
        assert_eq!(unspell(literal), literal);
        assert_eq!(unspell("<U+x<U+200B>"), "<U+x\u{200b}");
    }
}
