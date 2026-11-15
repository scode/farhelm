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
//!
//! Nothing here is optimistic. The list shows what the store holds; a write in flight shows nothing until the helm
//! pushes the result, which on the embedded helm is the same moment its reply arrives.

use std::collections::BTreeMap;

use farhelm_helm_client::{ApiPath, Method, PathError};
use farhelm_proto::launcher::{LaunchTemplate, quick_launch_eligible};
use farhelm_proto::ui_channel::{PREFERENCES_KEY, Preferences};
use gpui::App;
use serde_json::Value;

use crate::operations::{Command, Work};
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
    /// The bindings the store holds, or none before the helm pushed its preferences.
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
    patch(
        "quick_launch_template",
        marked.map_or(Value::Null, |name| Value::String(name.to_owned())),
    )
}

/// The write that replaces the number keys with `keys`, whole.
pub fn keys_command(keys: &BTreeMap<String, String>) -> Result<Command, PathError> {
    let map = keys
        .iter()
        .map(|(key, name)| (key.clone(), Value::String(name.clone())))
        .collect();
    patch("template_shortcuts", Value::Object(map))
}

/// `PUT /api/preferences` naming only `field`, so the helm leaves every other preference as it is.
fn patch(field: &str, value: Value) -> Result<Command, PathError> {
    let mut body = serde_json::Map::new();
    body.insert(field.to_owned(), value);
    Ok(Command::new(
        Work::Save,
        Method::Put,
        ApiPath::api(&["preferences"])?,
        Some(Value::Object(body)),
        "PUT /api/preferences",
    ))
}

/// Whether `template` may be given ⚡: an agent launch needs an agent type, a command launch a nonblank command, judged
/// by the shared rule the helm applies to an agent's write too. A marked template that no longer qualifies can still be
/// unmarked, which is the caller's to allow.
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
             ⚡. Can't mark this one: an agent launch needs an agent; a command launch needs a nonblank command."
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

/// The number-key button's text: the bound chord, or the old placeholder with no key ("Ctrl+Shift+ –", "⌘ –").
pub fn key_button_text(bound: Option<&str>) -> String {
    super::number_key_name(bound.unwrap_or(" –"))
}
