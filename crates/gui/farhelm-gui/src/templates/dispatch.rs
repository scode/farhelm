//! The quick-launch dispatcher: what every template shortcut does once it knows which templates it names.
//!
//! SPEC.md, "Creation": "Quick launch", "Template number shortcuts" and "Quick-launch sequences" share one path, and
//! "Keyboard focus" says what a launch's completion may do once the user has moved on. Ported from the old app's
//! `quick_launch` in `list/view.rs`, `QuickLaunchOrigin` and `PartialTemplateBanner` in `launch_sequence.rs`, and the
//! auto-launch and shortcut-batch parts of `list/create_form.rs`, at reference commit `24bc0580`. The old dispatcher's
//! worker queue, press snapshots, opening generations, modal epochs and fresh template reads are not ported: GPUI hands
//! each press over in order on one thread, templates and bindings come from the store the helm pushes, and the UI
//! channel's ordering keeps a launch from overtaking a template save sent before it.
//!
//! Here's the TLDR. Every shortcut piece calls [`dispatch`] with its [`Origin`] and the [`Reference`]s it names, from
//! the handler of the user's press, which is the answer the launch's completion counts from:
//!
//! - **Where the press lands.** In the session launcher, when it is the dialog holding the keyboard, the press applies
//!   to its draft and leaves Launch to the user. On the page it launches or opens New. Under any other dialog or menu
//!   it does nothing (SPEC.md: "Other visible dialogs keep the keyboard"), and while the launcher is busy (a quick
//!   launch being prepared, a launch pending, a refused launch waiting behind its notice, a host change in flight) it is
//!   taken and changes nothing.
//! - **Resolving.** Each reference becomes the template the store holds under that name now. On the page, a ⚡ or number
//!   key with nothing usable bound opens Templates with its hint, and a name the store no longer has opens New with a
//!   refusal saying so; in the launcher both are refusals shown there, and nothing opens. While the helm reports its
//!   templates unavailable, every press is refused with the old app's words for a failed read, and so is a ⚡ or
//!   number-key press while the shortcut bindings cannot be known (the preferences unavailable, or not arrived): the
//!   old app read both afresh for each press and refused when either read failed. Every refusal, a sequence's
//!   included, is a [`Refusal`] routed by one function ([`refused`]).
//! - **Applying.** The templates are applied all or nothing, through [`super::apply`], the only path by which a template
//!   changes a draft, and always against the model catalog the launcher read: in the launcher to its draft, each press
//!   or completed sequence its own refusal unit; on the page to New's prefill, in a launcher prepared out of sight
//!   while its catalog read runs (`launcher::view::prepare_unseen`), which, for a launch, then also waits out of sight
//!   for the host's launch history while New's folder default is pending and the templates chose neither a folder nor
//!   a managed checkout.
//! - **Launching or showing** (the page). When the templates together supply what their launch kind needs (the shared
//!   `stacked_quick_launch_eligible` fold), the draft is started without drawing the launcher, through its ordinary
//!   submit path; otherwise New is shown with the draft and the banner the origin calls for ([`incomplete_banner`]). A
//!   refusal shows New on its prefill saying why. From the submit on, the outcome is the launcher's: success opens the
//!   session unless the user moved on, and a refusal brings the launcher back with its draft or offers it in a notice.
//!   Nothing here watches the reply. A launch started or New shown from the page also brings the sidebar back from
//!   its Hosts panel to Sessions (`session_list::show_sessions`), as the old app's quick launch did through the list's
//!   New, so the session the press started is where the user can see it; Templates opened with a hint leaves the
//!   panel as it was.
//!
//! On the page a press waits for the catalog. A press that launches out of sight then also waits for the host's launch
//! history while New's folder default is pending (no session open on the host) and the templates chose neither a
//! folder nor a managed checkout: its newest plain-folder launch names the folder, and a failed read shows New saying
//! so (`launcher::view`'s quick-launch folder wait). A press that shows New does not wait for it there. Presses that
//! land in the launcher while its catalog read is out wait for it in order (`Launcher::apply_batch`). The templates
//! and bindings are resolved at the press; the hosts, remembered wrapper prefixes and default folders they are applied
//! against are read from the store when they apply, which for a press that waited is a frame or two later and
//! fresher.
//!
//! The banner New opens with goes about six seconds after the templates applied, or at the user's first edit (SPEC.md,
//! "Template number shortcuts", which the sequence's banner follows); the launcher keeps that deadline
//! (`launcher::view::BANNER_SHOWS_FOR`).
//!
//! A pick in the quick switcher's templates section ([`pick_from_switcher`]) does not go through [`dispatch`]: it opens
//! New at once and applies the template there as a press in the open launcher does, never launching.

use farhelm_proto::launcher::{LaunchTemplate, stacked_quick_launch_eligible};
use farhelm_proto::ui_channel::{Availability, Collection, HostView, PREFERENCES_KEY, Preferences};
use gpui::{App, Entity, Window};

use super::sequence::{self, Binding, Refusal};
use super::shortcuts::Bindings;
use super::summary::Part;
use super::{SEQUENCE_PREFIX_NAME, apply};
use crate::focus::Mark;
use crate::host_text::{HostSentence, HostText};
use crate::launcher::draft::{Catalog, LaunchDraft};
use crate::launcher::view::{BatchApplied, Launcher, Note, Prepared};
use crate::launcher::{self, entry};
use crate::{layers, local_state, session, session_list};

/// Where a quick launch came from. It decides only the banner New shows when the templates cannot launch on their own:
/// a number key's explains the sequence prefix, a sequence's speaks of the combined setup (it already used the prefix),
/// and the rest say the template needs more choices (SPEC.md; the old `QuickLaunchOrigin::incomplete_banner`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Origin {
    /// The ⚡ chord.
    Mark,
    /// A number key's chord.
    NumberKey,
    /// A quick-launch sequence's Enter.
    Sequence,
    /// A pick in the Cmd+J template list.
    TemplateList,
    /// The quick switcher: its Quick launch row, or a pick in its templates section.
    Switcher,
}

/// One template a quick launch names, in the order it applies.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Reference {
    /// The template holding ⚡, as the store says now.
    Mark,
    /// The template holding this number key, named by its digit as the bindings map spells it, as the store says now.
    Key(String),
    /// The template of exactly this name: a sequence's resolved word or captured binding, or a list's pick. A name the
    /// store no longer has is refused rather than standing in for anything else.
    Name(String),
}

impl Reference {
    /// Whether resolving this reference reads the store's ⚡ mark or number keys, so it cannot be resolved while they
    /// are unknown.
    fn reads_bindings(&self) -> bool {
        match self {
            Reference::Mark | Reference::Key(_) => true,
            Reference::Name(_) => false,
        }
    }
}

/// Quick launch `references`, pressed from `origin`, in `window`: the module docs' path. Call it from the handler of the
/// user's press (or a sequence's Enter, or a list's pick).
pub fn dispatch(origin: Origin, references: &[Reference], window: &mut Window, cx: &mut App) {
    if references.is_empty() || refused_unavailable(window, cx) {
        return;
    }
    let Some(bindings) = known_bindings(references, window, cx) else {
        return;
    };
    if let Some(launcher) = entry::open_launcher(window, cx) {
        in_launcher(&launcher, references, &bindings, window, cx);
    } else if on_page(window, cx) {
        on_the_page(origin, references, &bindings, window, cx);
    }
}

/// Show `refusal` where the shortcut was pressed, changing no draft: in the open launcher as its refusal with
/// [`Refusal::launcher_sentence`] (SPEC.md: "A missing mark or failed read shows its refusal where `tl:` does, without
/// opening Templates"); on the page, Templates with the binding hint when [`Refusal::hint`] names one, otherwise New
/// on its prefill with "Not launched." and [`Refusal::sentence`]. Call it from the press's handler. Nothing happens
/// where [`dispatch`] would do nothing.
pub(super) fn refused(refusal: &Refusal, window: &mut Window, cx: &mut App) {
    if let Some(launcher) = entry::open_launcher(window, cx) {
        if launcher.read(cx).save_panel_open() {
            return;
        }
        let sentence = sentence(LAUNCHER_REFUSAL, &refusal.launcher_sentence());
        launcher.update(cx, |launcher, cx| {
            launcher.apply_batch(move |_, _, _| Err(sentence), window, cx)
        });
    } else if on_page(window, cx) {
        if let Some(hint) = refusal.hint() {
            let mark = Mark::take(cx);
            super::open_with_hint(hint, &mark, window, cx);
        } else {
            let note = Note::Refusal(not_launched(&refusal.sentence()));
            if let Some(draft) = entry::new_prefill(cx)
                && entry::open_with(draft, Some(note), window, cx)
            {
                session_list::show_sessions(window, cx);
            }
        }
    }
}

/// Refuse a template shortcut because the helm reports its templates unavailable, if it does ([`refused`] with
/// [`Refusal::Unavailable`]: the old app's "Couldn't load templates:" in the launcher and "Couldn't prepare quick
/// launch:" on the page, with the helm's words). Returns whether it refused, which a sequence's Enter asks first, so
/// that a word is never reported unknown only because the templates could not be read. Before the first snapshot the
/// window is inert, and nothing is refused here.
pub(super) fn refused_unavailable(window: &mut Window, cx: &mut App) -> bool {
    let Some(error) = unavailable(cx) else {
        return false;
    };
    refused(&Refusal::Unavailable(error), window, cx);
    true
}

/// The bindings `references` resolve through, or `None` once the press is refused because they cannot be known
/// ([`Bindings::read`]), as the old app refused a press whose fresh preference read failed, with that read's words
/// ("Couldn't prepare quick launch:" on the page, "Couldn't load templates:" in the launcher; it read the preferences
/// and the templates as one step). Acting on them instead would open Templates to mark a template that may hold ⚡
/// already, or say a key is unbound when it is not. A press that names its templates by name (a sequence's resolved
/// names, a list's pick) reads no binding and goes on with whatever the store holds, which it never consults.
fn known_bindings(references: &[Reference], window: &mut Window, cx: &mut App) -> Option<Bindings> {
    if !references.iter().any(Reference::reads_bindings) {
        return Some(Bindings::current(cx));
    }
    match Bindings::read(cx) {
        Ok(bindings) => Some(bindings),
        Err(unread) => {
            refused(&Refusal::from(unread), window, cx);
            None
        }
    }
}

/// The helm's words for why its templates are unavailable, if it reports them so; `None` before the first snapshot.
pub(super) fn unavailable(cx: &App) -> Option<String> {
    session::current_store(cx).and_then(|store| {
        match &store
            .read(cx)
            .collections()
            .meta(Collection::Templates)?
            .availability
        {
            Availability::Unavailable { error } => Some(error.clone()),
            Availability::Available => None,
        }
    })
}

/// The edit a taken `tl:` row makes in the open launcher (`launcher::view::search`): the template the store holds
/// under `name` when the batch applies, through [`apply::apply`] against the launcher's catalog, as a press of a
/// template shortcut there is (SPEC.md: "`tl:name` in the search box offers templates by name, and accepting one
/// applies it"). It keeps the launch kind unless the template sets one, as applying a template always does. Its
/// refusals are the old launcher's words for a `tl:` acceptance, changing nothing: a failed template read is
/// "Couldn't load templates: <the helm's words>; search tl: and retry", a name the store no longer has is "No
/// template is named \"<name>\"; it may have been deleted", and a template that does not apply says why in
/// [`apply::apply`]'s words alone.
pub(crate) fn search_row_batch(
    name: String,
) -> impl FnOnce(&mut LaunchDraft, Catalog<'_>, &App) -> Result<BatchApplied, HostSentence> + 'static
{
    move |draft, catalog, cx| {
        if let Some(error) = unavailable(cx) {
            return Err(sentence(
                LAUNCHER_REFUSAL,
                &[
                    Part::Words("Couldn't load templates: ".to_owned()),
                    Part::Host(error),
                    Part::Words("; search tl: and retry".to_owned()),
                ],
            ));
        }
        let Some(template) = stored_templates(cx)
            .into_iter()
            .find(|template| template.name == name)
        else {
            return Err(sentence(
                LAUNCHER_REFUSAL,
                &[
                    Part::Words("No template is named \"".to_owned()),
                    Part::Host(name),
                    Part::Words("\"; it may have been deleted".to_owned()),
                ],
            ));
        };
        apply_in_launcher(draft, &[template], catalog, cx)
    }
}

/// Apply `templates` to the open launcher's draft as one batch ([`apply_from_store`]), saying the refusal, if any, in
/// the launcher's words: a template press's or a taken `tl:` row's.
fn apply_in_launcher(
    draft: &mut LaunchDraft,
    templates: &[LaunchTemplate],
    catalog: Catalog<'_>,
    cx: &App,
) -> Result<BatchApplied, HostSentence> {
    apply_from_store(draft, templates, catalog, cx)
        .map(|applied| BatchApplied {
            notice: applied.notice,
            note: None,
        })
        .map_err(|refusal| sentence(LAUNCHER_REFUSAL, &[Part::Host(refusal)]))
}

/// A pick of the template named `name` in the quick switcher's templates section: open New on its prefill at once and
/// apply the template to it, never launching (SPEC.md: "Picking one opens New with its edits applied, without using
/// the query as the session name ... Neither name nor template pick launches a session"), as the old switcher opened
/// New and then accepted the template there. The template is applied as a press in the open launcher is
/// ([`Launcher::apply_batch`]), so it waits for New's catalog read; until then New shows its prefill. A template that
/// cannot launch on its own is followed by the needs-more-choices banner once it applies. The template is the one the
/// store holds under `name` at the pick.
///
/// Its refusals are the old switcher pick's, shown in the open New with no "Not launched.", since nothing was
/// launching: a failed template read is "Couldn't load templates: <the helm's words>; search tl: and retry", a name
/// the store no longer has is "No template is named \"<name>\"; it may have been deleted", and a template that does
/// not apply says why in [`apply::apply`]'s words alone, changing nothing.
///
/// Like [`dispatch`] it does nothing while a dialog or menu holds the keyboard or the launcher is busy (a launch
/// pending, a quick launch being prepared, a refused launch waiting behind its notice, a host change in flight), as the
/// switcher's New row does then; so the switcher closes itself first and calls this from the handler of the user's
/// pick.
pub fn pick_from_switcher(name: &str, window: &mut Window, cx: &mut App) {
    if !on_page(window, cx) {
        return;
    }
    let Some(draft) = entry::new_prefill(cx) else {
        return;
    };
    // New with the old switcher pick's refusal, for a pick that cannot apply at all.
    let refused = |draft: LaunchDraft, parts: &[Part], window: &mut Window, cx: &mut App| {
        let note = Note::Refusal(sentence(PAGE_REFUSAL, parts));
        entry::open_with(draft, Some(note), window, cx);
    };
    if let Some(error) = unavailable(cx) {
        let parts = [
            Part::Words("Couldn't load templates: ".to_owned()),
            Part::Host(error),
            Part::Words("; search tl: and retry".to_owned()),
        ];
        return refused(draft, &parts, window, cx);
    }
    let Some(template) = stored_templates(cx)
        .into_iter()
        .find(|template| template.name == name)
    else {
        let parts = [
            Part::Words("No template is named \"".to_owned()),
            Part::Host(name.to_owned()),
            Part::Words("\"; it may have been deleted".to_owned()),
        ];
        return refused(draft, &parts, window, cx);
    };
    // `on_page` above is all that keeps a pick off a refused launch waiting behind its notice: `open_with` would bring
    // that one back instead of opening this draft.
    if !entry::open_with(draft, None, window, cx) {
        return;
    }
    let Some(launcher) = entry::open_launcher(window, cx) else {
        return;
    };
    let templates = vec![template];
    // Whether the template can launch on its own does not depend on the catalog, so the banner is decided now.
    let banner = (!stacked_quick_launch_eligible(
        templates.iter().map(|template| &template.fields),
    ))
    .then(|| {
        Note::Banner(sentence(
            "quick-launch-banner",
            &incomplete_banner(Origin::Switcher, &templates),
        ))
    });
    launcher.update(cx, |launcher, cx| {
        launcher.apply_batch(
            move |draft, catalog, cx| {
                apply_from_store(draft, &templates, catalog, cx)
                    .map(|applied| BatchApplied {
                        notice: applied.notice,
                        note: banner,
                    })
                    .map_err(|refusal| sentence(PAGE_REFUSAL, &[Part::Host(refusal)]))
            },
            window,
            cx,
        )
    });
}

/// The banner New shows when `templates`, applied for a quick launch from `origin`, do not make a launch on their own, in
/// the old app's words. A number key and the ⚡ chord, Cmd+J and the switcher name their one template; a sequence speaks
/// of the whole setup, however many templates it queued.
pub fn incomplete_banner(origin: Origin, templates: &[LaunchTemplate]) -> Vec<Part> {
    let name = || {
        Part::Host(
            templates
                .first()
                .map(|template| template.name.clone())
                .unwrap_or_default(),
        )
    };
    match origin {
        Origin::Sequence => vec![Part::Words(
            "This sequence doesn't make a complete setup; complete it below.".to_owned(),
        )],
        Origin::NumberKey => vec![
            name(),
            Part::Words(format!(
                " cannot launch on its own. Use {SEQUENCE_PREFIX_NAME} to combine template keys, then Enter to \
                 launch, or complete this setup below."
            )),
        ],
        Origin::Mark | Origin::TemplateList | Origin::Switcher => vec![
            name(),
            Part::Words(" needs more choices before it can launch; complete it below.".to_owned()),
        ],
    }
}

/// The element ids of a refusal: one shown on New opened from the page, one shown in the open launcher.
const PAGE_REFUSAL: &str = "quick-launch-refusal";
const LAUNCHER_REFUSAL: &str = "launcher-shortcut-refusal";

// ---- The page --------------------------------------------------------------------------------------------------------

/// Whether a quick launch pressed now may act on the page: a UI session is connected, no dialog or menu keeps the
/// keyboard, and the launcher is free ([`entry::quick_launch_available`]). A refused press is still taken by its
/// handler; it only changes nothing.
fn on_page(window: &Window, cx: &mut App) -> bool {
    session::current_store(cx).is_some()
        && !layers::focus_layer_open(window, cx)
        && entry::quick_launch_available(cx)
}

/// The page's half of [`dispatch`]: resolve, then prepare New's prefill out of sight, apply once the catalog is read,
/// and launch unseen or show New saying why not.
fn on_the_page(
    origin: Origin,
    references: &[Reference],
    bindings: &Bindings,
    window: &mut Window,
    cx: &mut App,
) {
    let templates = match resolve(references, bindings, &stored_templates(cx)) {
        Ok(templates) => templates,
        Err(refusal) => return refused(&refusal, window, cx),
    };
    let Some(draft) = entry::new_prefill(cx) else {
        return;
    };
    let launches = stacked_quick_launch_eligible(templates.iter().map(|template| &template.fields));
    let finish = move |draft: &mut LaunchDraft, catalog: Catalog<'_>, cx: &App| {
        match apply_from_store(draft, &templates, catalog, cx) {
            // The draft is untouched: New shows its prefill.
            Err(refusal) => Prepared::Show {
                note: Note::Refusal(not_launched(&[Part::Host(refusal)])),
                notice: None,
            },
            Ok(applied) if launches => Prepared::Launch {
                notice: applied.notice.flatten(),
            },
            Ok(applied) => Prepared::Show {
                note: Note::Banner(sentence(
                    "quick-launch-banner",
                    &incomplete_banner(origin, &templates),
                )),
                notice: applied.notice.flatten(),
            },
        }
    };
    if launcher::view::prepare_unseen(draft, Mark::take(cx), finish, window, cx) {
        session_list::show_sessions(window, cx);
    }
}

// ---- The open launcher -----------------------------------------------------------------------------------------------

/// The launcher's half of [`dispatch`]: `references` applied to the open `launcher`'s draft as one batch, or refused
/// there with the launcher's words, changing nothing (SPEC.md: "Consecutive presses apply in order; a refusal changes
/// nothing from that press and does not discard a later press"). The references are resolved at the press; the
/// templates are checked against the catalog the launcher has read, once it has, and the store as it is then.
fn in_launcher(
    launcher: &Entity<Launcher>,
    references: &[Reference],
    bindings: &Bindings,
    window: &mut Window,
    cx: &mut App,
) {
    // SPEC.md, "Quick launch": "while saving a template, the chord is consumed without changing the draft"; Save as
    // template's panel captures the draft as it stands, so a press while it is open is taken and changes nothing.
    if launcher.read(cx).save_panel_open() {
        return;
    }
    let resolved = resolve(references, bindings, &stored_templates(cx));
    launcher.update(cx, |launcher, cx| {
        launcher.apply_batch(
            move |draft, catalog, cx| {
                let templates = resolved
                    .map_err(|refusal| sentence(LAUNCHER_REFUSAL, &refusal.launcher_sentence()))?;
                apply_in_launcher(draft, &templates, catalog, cx)
            },
            window,
            cx,
        )
    });
}

// ---- Resolving and applying ------------------------------------------------------------------------------------------

/// The templates `references` name, in order, from `templates` (the store's) and `bindings` (the store's ⚡ mark and
/// number keys), or why not, as the refusal a sequence would give.
///
/// A missing name wins over any unusable binding, as in the old dispatcher, which looked for one before falling back to
/// the binding hint; among unusable bindings the first in order is reported, named alone when it was the whole batch. A
/// ⚡ or key bound to a name no template has is as unusable as an unbound one: SPEC.md's "With no usable mark it opens
/// Templates with an explanation". Names match exactly; a binding names one template, never a case variant of it.
pub(super) fn resolve(
    references: &[Reference],
    bindings: &Bindings,
    templates: &[LaunchTemplate],
) -> Result<Vec<LaunchTemplate>, Refusal> {
    let find = |name: &str| templates.iter().find(|template| template.name == name);
    let mut unbound = None;
    let mut found = Vec::with_capacity(references.len());
    for reference in references {
        let (bound, unusable) = match reference {
            Reference::Name(name) => match find(name) {
                Some(template) => {
                    found.push(template.clone());
                    continue;
                }
                None => return Err(Refusal::Missing(name.clone())),
            },
            Reference::Mark => (bindings.marked.as_deref(), sequence::Reference::Mark),
            Reference::Key(key) => (
                bindings.keys.get(key).map(String::as_str),
                sequence::Reference::Key(key.clone()),
            ),
        };
        match bound.and_then(find) {
            Some(template) => found.push(template.clone()),
            None => {
                unbound.get_or_insert(unusable);
            }
        }
    }
    match unbound {
        Some(reference) => Err(Refusal::Unbound(Binding {
            reference,
            alone: references.len() == 1,
        })),
        None => Ok(found),
    }
}

/// Every template the store holds, in the helm's order; none without a UI session.
fn stored_templates(cx: &App) -> Vec<LaunchTemplate> {
    session::current_store(cx)
        .map(|store| {
            store
                .read(cx)
                .collections()
                .ordered::<LaunchTemplate>()
                .into_iter()
                .cloned()
                .collect()
        })
        .unwrap_or_default()
}

/// Apply `templates` to `draft` with [`apply::apply`], against `catalog` and what the store holds now: the hosts, the
/// remembered wrapper prefixes, and New's folder for a host a template switches to. All of them or, with its refusal
/// sentence, none.
fn apply_from_store(
    draft: &mut LaunchDraft,
    templates: &[LaunchTemplate],
    catalog: Catalog<'_>,
    cx: &App,
) -> Result<apply::Applied, String> {
    let Some(store) = session::current_store(cx) else {
        // A reset retired the UI session, and the launcher this batch was for goes with it.
        return Err("The window lost its connection to the helm".to_owned());
    };
    let collections = store.read(cx).collections();
    let hosts: Vec<HostView> = collections
        .ordered::<HostView>()
        .into_iter()
        .cloned()
        .collect();
    let remembered_wrappers = collections
        .get::<Preferences>(PREFERENCES_KEY)
        .map(|preferences| preferences.remembered_wrapper_prefixes.clone())
        .unwrap_or_default();
    let selected = local_state::selected(cx);
    let default_folder = |host| entry::default_folder(collections, selected.as_deref(), host);
    let context = apply::Context {
        hosts: &hosts,
        catalog,
        remembered_wrappers: &remembered_wrappers,
        default_folder: &default_folder,
    };
    apply::apply(draft, templates, &context)
}

/// `parts` as a host sentence under `id`: the app's words as words, template names, values and the helm's words as host
/// text, so they are drawn escaped (an [`apply::apply`] refusal names its template, so it goes in as host text whole).
fn sentence(id: &'static str, parts: &[Part]) -> HostSentence {
    extended(HostSentence::new(id), parts)
}

/// A refusal shown on New opened from the page, "Not launched." first: the quick launch did not start.
fn not_launched(parts: &[Part]) -> HostSentence {
    extended(
        HostSentence::new(PAGE_REFUSAL).text("Not launched. "),
        parts,
    )
}

/// `sentence` with `parts` appended, as [`sentence`] reads them.
fn extended(sentence: HostSentence, parts: &[Part]) -> HostSentence {
    parts.iter().fold(sentence, |sentence, part| match part {
        Part::Words(words) => sentence.text(words.clone()),
        Part::Host(text) | Part::Mono(text) => sentence.host(&HostText::from_host(text.clone())),
    })
}
