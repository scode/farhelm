//! The ⚡ mark and the number keys: which template holds them, how a change is written, and how the Templates list
//! words them.
//!
//! SPEC.md, "Quick launch" and "Template number shortcuts". Both are shared preferences on the helm, not template
//! fields: the mark is one nullable template name (`quick_launch_template`), the keys one map from a digit to a template
//! name (`template_shortcuts`). Ported from the old app's `list/templates.rs` (`bind_number`, `NumberShortcutMenu` and
//! the bolt's and key's hover help) at reference commit `24bc0580`.
//!
//! Here's the TLDR of the write rules, which SPEC.md's "One GUI at a time" confirmed on 2026-10-21:
//!
//! - **The mark** is written as the name it now holds, or `null` to clear it, and only when the store does not already
//!   say so: a click meant to mark a template an agent has just marked writes nothing.
//! - **The keys** are written as the whole map, built from the map the store holds at the moment of the click, so a
//!   concurrent change to another key can be lost, which SPEC.md accepts ("a GUI save, which re-reads the bindings
//!   first, can undo an agent's or the command line's binding change that lands between that read and its write").
//!   The store is that re-read: it holds what the helm pushed last, and the channel delivers a change before the reply
//!   of anything sent after it.
//! - **Unknown bindings refuse.** While the helm reports its preferences unavailable, or before they have arrived,
//!   nothing writes them or acts on them: the writes, the ⚡ chord and the number keys, a sequence's ⚡ and number keys,
//!   and a rename's moves are refused with the reason ([`Bindings::read`]). Only drawing uses whatever the store holds.
//!
//! Nothing here is optimistic. The list shows what the store holds; a write in flight shows nothing until the helm
//! pushes the result, which on the embedded helm is the same moment its reply arrives.

use std::collections::BTreeMap;

use farhelm_helm_client::PathError;
use farhelm_proto::launcher::{LaunchTemplate, quick_launch_eligible};
use farhelm_proto::ui_channel::{Availability, Collection, PREFERENCES_KEY, Preferences};
use gpui::App;

use crate::host_text::{HostSentence, HostText};
use crate::operations::Command;
use crate::session;

/// The ten keys, in the order the menu offers them: 1 to 9, then 0 (SPEC.md: "one of ten shared keys, 1–9 then 0").
pub const KEYS: [&str; 10] = ["1", "2", "3", "4", "5", "6", "7", "8", "9", "0"];

/// The mark and the keys as the store holds them now.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Bindings {
    /// The template holding ⚡, by name; it need not exist (an agent may have marked a name, or a template may have
    /// been renamed elsewhere).
    pub marked: Option<String>,
    /// Digit to template name, as the helm stores it.
    pub keys: BTreeMap<String, String>,
}

impl Bindings {
    /// The bindings the store holds, or none before the helm pushed its preferences: for drawing only. Anything that
    /// acts on the bindings (a write, a press, a rename's moves) reads them through [`Bindings::read`], which says when
    /// this answer is empty or stale.
    pub fn current(cx: &App) -> Bindings {
        session::current_store(cx)
            .and_then(|store| {
                store
                    .read(cx)
                    .collections()
                    .get::<Preferences>(PREFERENCES_KEY)
                    .map(|preferences| Bindings {
                        marked: preferences.quick_launch_template.clone(),
                        keys: preferences.template_shortcuts.clone(),
                    })
            })
            .unwrap_or_default()
    }

    /// The bindings the store holds, or why they cannot be known ([`Unread`]): the helm reports its preferences
    /// unavailable, or they have not arrived. Whatever acts on the bindings, rather than only drawing
    /// them, reads them through this, because [`Bindings::current`] is empty before the first push and the last pushed
    /// value after a failed read, and either can look like a real answer. A ⚡ chord on empty bindings opens Templates
    /// to mark a template that may already be marked; a number-key write built from them replaces the whole stored map;
    /// a rename that sees no ⚡ deletes the old name and the mark with it. The old app read the preferences afresh for
    /// each of these and refused on a failed read, with the read's words.
    pub fn read(cx: &App) -> Result<Bindings, Unread> {
        // Preferences that have not arrived prove nothing about the bindings, as a missing template list proves nothing
        // about the names.
        let store = session::current_store(cx).ok_or(Unread::NotArrived)?;
        match store
            .read(cx)
            .collections()
            .meta(Collection::Preferences)
            .map(|meta| &meta.availability)
        {
            Some(Availability::Unavailable { error }) => Err(Unread::Unavailable(error.clone())),
            Some(Availability::Available) => Ok(Bindings::current(cx)),
            None => Err(Unread::NotArrived),
        }
    }

    /// Whether `name` holds ⚡.
    pub fn is_marked(&self, name: &str) -> bool {
        self.marked.as_deref() == Some(name)
    }

    /// The key `name` holds, if any. A template holds at most one; if the stored map somehow gives one several, the
    /// first in the menu's order wins, so the button and the menu agree.
    pub fn key_of(&self, name: &str) -> Option<&'static str> {
        KEYS.into_iter()
            .find(|key| self.keys.get(*key).map(String::as_str) == Some(name))
    }

    /// The map after giving `name` the key `key` (or none): the key `name` held is freed, and `key` moves to `name`
    /// from whichever template held it (SPEC.md: "choosing one moves it"; "A template has at most one key and each key
    /// has at most one template").
    pub fn keys_with(&self, name: &str, key: Option<&str>) -> BTreeMap<String, String> {
        let mut keys: BTreeMap<String, String> = self
            .keys
            .iter()
            .filter(|(_, holder)| holder.as_str() != name)
            .map(|(key, holder)| (key.clone(), holder.clone()))
            .collect();
        if let Some(key) = key {
            keys.insert(key.to_owned(), name.to_owned());
        }
        keys
    }
}

/// Why the bindings cannot be known ([`Bindings::read`]), kept apart so a refusal draws the helm's words as host text
/// and the app's own as its words.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Unread {
    /// The helm reports its preferences unavailable, in these words.
    Unavailable(String),
    /// The helm has not pushed its preferences yet ([`Unread::NOT_ARRIVED`]).
    NotArrived,
}

impl Unread {
    /// The app's words for [`Unread::NotArrived`], ending a refusal's sentence.
    pub const NOT_ARRIVED: &str = "the shortcut bindings have not arrived yet";

    /// `sentence` with the reason appended: the helm's words as host text, the app's own as words.
    pub fn appended_to(&self, sentence: HostSentence) -> HostSentence {
        match self {
            Unread::Unavailable(error) => sentence.host(&HostText::from_host(error.clone())),
            Unread::NotArrived => sentence.text(Unread::NOT_ARRIVED),
        }
    }
}

/// The number keys after a rename of `old` to `new`: every key `old` held is held by `new` instead, and nothing else
/// changes (SPEC.md: "Rename keeps a key"; the old app's `rename_number_binding`).
pub fn keys_renamed(
    keys: &BTreeMap<String, String>,
    old: &str,
    new: &str,
) -> BTreeMap<String, String> {
    keys.iter()
        .map(|(key, holder)| {
            let holder = if holder == old { new } else { holder.as_str() };
            (key.clone(), holder.to_owned())
        })
        .collect()
}

/// What a click on `name`'s bolt writes, given whether the bolt was drawn lit when the user clicked it and what the
/// store holds now: `Some(new mark)` to write, or `None` when the store already says what the click asks for. The intent
/// comes from the drawn bolt, never from the store at the moment the click is handled, so a click meant to mark a
/// template an agent has just marked writes nothing rather than clearing that mark (SPEC.md, "One GUI at a time": "A
/// GUI write must not undo a change an agent or the command line made").
pub fn mark_write<'a>(
    name: &'a str,
    was_lit: bool,
    stored: Option<&str>,
) -> Option<Option<&'a str>> {
    // The click is about this template alone: whether it should hold the mark. Comparing with whether it holds it now
    // (rather than with the whole stored value) keeps a click that meant to clear this template's mark from clearing a
    // mark an agent has meanwhile moved to another template, as the old app's `Bolt(name, !lit)` intent did.
    let mark = !was_lit;
    let holds = stored == Some(name);
    (mark != holds).then_some(mark.then_some(name))
}

/// The write that makes `marked` the ⚡ template, or clears the mark with `None`: a sparse patch of the preference
/// row's `quick_launch_template` field.
pub fn mark_command(marked: Option<&str>) -> Result<Command, PathError> {
    crate::settings::preference_command(serde_json::json!({ "quick_launch_template": marked }))
}

/// The write that replaces the number keys with `keys`, whole.
pub fn keys_command(keys: &BTreeMap<String, String>) -> Result<Command, PathError> {
    crate::settings::preference_command(serde_json::json!({ "template_shortcuts": keys }))
}

/// The bolt uses the same eligibility rule as an agent's template write. Agent needs a built-in or registered custom
/// agent. Command needs a nonblank command. A marked template that no longer qualifies can still be unmarked.
pub fn may_mark(template: &LaunchTemplate) -> bool {
    quick_launch_eligible(&template.fields)
}

/// The bolt's hover help, the old list's words: what ⚡ does, that one template holds it, and what a click would do.
pub fn bolt_help(marked: bool, eligible: bool) -> String {
    let chord = super::QUICK_LAUNCH_SHORTCUT_NAME;
    if marked {
        format!(
            "{chord} starts a session from this template right away, without opening New. Only one template can hold ⚡. \
             Click to turn off."
        )
    } else if eligible {
        format!(
            "{chord} starts a session from the ⚡ template right away, without opening New. Only one template can hold \
             ⚡. Click to mark this one."
        )
    } else {
        format!(
            "{chord} starts a session from the ⚡ template right away, without opening New. Only one template can hold \
             ⚡. Can't mark this one. Agent needs a built-in or registered custom agent. Command needs a nonblank command."
        )
    }
}

/// The number-key button's hover help, the old list's words: what a key does for this template (which depends on
/// whether it can launch on its own), that a key belongs to one template, and the current choice.
pub fn key_help(launchable: bool, bound: Option<&str>) -> String {
    let behavior = if launchable {
        "A number shortcut starts a session from this template right away, without opening New."
            .to_owned()
    } else {
        format!(
            "This template cannot launch on its own. Its key opens New with it applied; {} starts a sequence to \
             combine it with other templates.",
            super::SEQUENCE_PREFIX_NAME
        )
    };
    let state = match bound {
        Some(key) => format!(
            "{} is set; click to change or remove it.",
            super::number_key_name(key)
        ),
        None => "Click to give this template a number key.".to_owned(),
    };
    format!("{behavior} Only one template can hold each key. {state}")
}

/// The fixed-width cap shows a digit on Linux and Cmd plus the digit on macOS. The plus opens the same menu
/// when no key is assigned. See [`key_help`] for the full binding.
pub fn key_button_text(bound: Option<&str>) -> String {
    match bound {
        None => "+".to_owned(),
        Some(key) if cfg!(target_os = "macos") => super::number_key_name(key),
        Some(key) => key.to_owned(),
    }
}
