//! Templates as a list shows them: the rows for a query, and what the lists that show them do with a pick.
//!
//! SPEC.md, "Desktop window chrome": the quick switcher's templates section matches "using the session launcher's own
//! template search; `tl:` alone lists every template", and the Cmd+J list "filters template names as `tl:` does,
//! without a prefix", with each row showing "the template name, summary and its ⚡ or number shortcut". So there is one
//! search, [`rows`], which the switcher's section, the launcher's `tl:` search and the Cmd+J list all call. Ported from
//! the old launcher's `template_search_results` and `scoped_query` (`launch_composer.rs`) and the switcher's template
//! rows (`list/quick_switcher.rs`) at reference commit `24bc0580`.
//!
//! The `tl:` prefix is the caller's to strip ([`template_query`]): the switcher must find it anyway to decide that
//! templates replace its sessions, the launcher's search reads several labels of which `tl:` is one, and the Cmd+J list
//! has no prefix. [`rows`] takes only the text to match. The Cmd+J list's highlight of a name exactly equal to the
//! query is the list's rule, made over the rows this returns, which keep the store's order and the names unchanged.
//!
//! The drawing and the words are the caller's (the sidebar draws the switcher, in the old switcher's words). The rest of
//! this module is what the switcher needs from the templates side for its section and its Quick launch row: what a
//! pick does, and the row's label and action.

use farhelm_proto::launcher::LaunchTemplate;
use gpui::{App, Window};

use super::dispatch::{self, Origin, Reference};
use super::shortcuts::Bindings;
use super::summary::{self, Part};
use crate::host_text::{HostSentence, HostText};
use crate::session;

/// One template as a list row may show it. A row carries the name, the summary, the ⚡ mark and the number key, and the
/// caller decides what to draw: the old switcher's templates section drew the name and summary only, and the Cmd+J
/// list added the ⚡ and number-key badges (SPEC.md names the shortcuts for Cmd+J's rows alone).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    /// The template's name, exactly as stored: host text, to be drawn escaped, and the name a pick passes back.
    pub name: String,
    /// What the template sets ([`summary::parts`]), as the Templates list shows it.
    pub summary: Vec<Part>,
    /// Whether the template holds ⚡.
    pub marked: bool,
    /// The number key the template holds, if any ("1" to "9", "0").
    pub key: Option<&'static str>,
}

/// The text a launcher-search or switcher `query` asks templates for: what follows a leading `tl:`, trimmed, or `None`
/// when the query does not start with that label. The label is matched as the old search matched it: the trimmed query
/// up to its first colon, `tl` in any ASCII case. So `tl:` alone asks for every template, and `TL: build` for `build`.
pub fn template_query(query: &str) -> Option<&str> {
    let (label, value) = query.trim().split_once(':')?;
    label.eq_ignore_ascii_case("tl").then(|| value.trim())
}

/// The templates whose names contain `filter` (trimmed), in the store's order, or why the templates cannot be listed.
///
/// Matching is the old search's: a substring of the name after Unicode lowercasing both, so `Build` finds `nightly
/// build` and an empty filter finds every template. Templates and the ⚡ and number-key bindings are read from the store
/// now, so a list redrawn on each push stays current. While the helm reports its templates unavailable, the answer is
/// the old switcher's "Couldn't load templates:" with the helm's words, for the caller to show where it shows rows.
/// Before the first snapshot the store holds nothing, and the answer is no rows.
pub fn rows(filter: &str, cx: &App) -> Result<Vec<Row>, HostSentence> {
    if let Some(error) = dispatch::unavailable(cx) {
        return Err(HostSentence::new("template-rows-unavailable")
            .text("Couldn't load templates: ")
            .host(&HostText::from_host(error)));
    }
    let Some(store) = session::current_store(cx) else {
        return Ok(Vec::new());
    };
    let collections = store.read(cx).collections();
    let wanted = filter.trim().to_lowercase();
    let bindings = Bindings::current(cx);
    Ok(collections
        .ordered::<LaunchTemplate>()
        .into_iter()
        .filter(|template| template.name.to_lowercase().contains(&wanted))
        .map(|template| Row {
            name: template.name.clone(),
            summary: summary::parts(&template.fields),
            marked: bindings.is_marked(&template.name),
            key: bindings.key_of(&template.name),
        })
        .collect())
}

// ---- The quick switcher ----------------------------------------------------------------------------------------------

/// A pick in the switcher's templates section ([`dispatch::pick_from_switcher`], which has its contract).
pub use super::dispatch::pick_from_switcher;

/// The Quick launch row's label: "Quick launch: " and the marked template's name (host text, as stored, whether or not
/// a template still has it), or "Mark a template" when none is marked, in the old words. The row draws the ⚡ chord
/// beside it (`super::QUICK_LAUNCH_SHORTCUT_NAME`). Read from the store each time the row draws.
pub fn quick_launch_row_label(cx: &App) -> HostSentence {
    let sentence = HostSentence::new("quick-switcher-quick-launch").text("Quick launch: ");
    match Bindings::current(cx).marked {
        Some(name) => sentence.host(&HostText::from_host(name)),
        None => sentence.text("Mark a template"),
    }
}

/// The Quick launch row's action: the ⚡ chord's quick launch, from the switcher (SPEC.md: the row "uses the direct
/// quick-launch path, or opens Templates to mark one"). It launches the marked template unseen, opens New when it needs
/// more choices, or opens Templates with the ⚡ hint when no usable template holds the mark. Call it once the switcher
/// has closed, from the handler of the user's pick, as for [`pick_from_switcher`].
pub fn quick_launch_from_switcher(window: &mut Window, cx: &mut App) {
    dispatch::dispatch(Origin::Switcher, &[Reference::Mark], window, cx);
}
