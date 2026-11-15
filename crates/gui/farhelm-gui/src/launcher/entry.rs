//! The launcher's entry points: the ordinary public functions other surfaces call to open New (the session list's New
//! button, the quick switcher's "New session named X" row, Cmd+N), what those surfaces need to draw their triggers,
//! and the YOLO question the sidebar's plain Replace asks ([`ask_yolo_for_replace`]).
//!
//! SPEC.md, "Creation" ("Working directory", "Host"), "Desktop window chrome" (Cmd+N) and the quick switcher's
//! paragraph. Ported from the old app's New handling in `list/view.rs` at reference commit `24bc0580`
//! (`effective_create_host`, the New button's guards and its tooltip).
//!
//! New is refused while a launch is pending: the old New button was disabled while the page's one operation token was
//! held, and SPEC.md keeps "a launch is pending" among what disables it ("Cmd+N ... unless a modal is open or New is
//! disabled"). It is refused too while one of this window's host-registry changes is in flight (SPEC.md: "Launches and
//! changes to the host registry still exclude each other"; `launch::host_change_in_flight` says which work counts), as
//! the old token's host changes disabled it. A refused launch waiting behind its notice is different: New brings that
//! launcher back, with its draft and refusal, rather than starting a fresh one, as the old app did (its New revealed a
//! returned launcher). SPEC.md leaves open whether that notice keeps New refused until it is opened; bringing the draft
//! back answers it without refusing anything.
//!
//! NOTE: New's prefill takes the open session's host and folder, else the helm's own host and `~`; the host picker
//! re-seeds an untouched folder from the same per-host default ([`default_folder`]). Not built yet, each with the piece
//! that brings it: the rest of SPEC.md's folder default (the newest recent setup's folder on that host) with recent
//! setups, and New preselecting an open environment session's recipe and place with the environments work; until then
//! an open session on a host that a sandbox or container owns prefills the helm's own host, since such a host is no
//! place for an unrelated session (the host picker leaves it out too). And the open session is read from the store, so
//! one the sidebar's filter or host choice leaves unlisted is not found, and New falls back to the helm's own host;
//! the old app kept the selected row's destination for that (its "New inherits a filtered selected destination"
//! case), and nothing in the app keeps an unlisted selected session's row yet.

use farhelm_helm_client::Collections;
use farhelm_proto::ui_channel::{
    HostStateView, HostView, PREFERENCES_KEY, Preferences, SessionRow,
};
use std::rc::Rc;

use farhelm_proto::SessionLaunch;
use gpui::{App, Window};

use super::draft::{LaunchDraft, remembered_permission};
use super::launch;
use super::view::open;
use super::view::yolo_question::{self, replace_owner};
use crate::focus::Mark;
use crate::host_text::HostText;
use crate::hosts::offers::HostKind;
use crate::prompt;
use crate::{layers, local_state, session};

/// New's hover text, for the button that opens it: it names the shortcut in the Mac app only, since the Linux app has
/// none (SPEC.md, "Desktop window chrome"). The old button's tooltip, word for word.
pub const NEW_SESSION_HELP: &str = if cfg!(target_os = "macos") {
    "New session: start an agent or a command on any host (⌘N)"
} else {
    "New session: start an agent or a command on any host"
};

/// Whether New can open now: not while a launch is pending, nor while one of this window's host-registry changes is
/// in flight, when the old New button was disabled too (its page-wide operation token covered launches and host
/// changes, though not a new host's probe, which the migration's orchestrator decided counts here). A surface
/// drawing a New trigger draws it disabled while this is `false`, reading it while it draws: the launch slot and the
/// operation store both notify when what it answers from changes.
pub fn new_available(cx: &mut App) -> bool {
    !launch::pending(cx) && !launch::host_change_in_flight(cx)
}

/// Open New in `window` with New's prefill (see the module docs), or bring back a refused launch waiting behind its
/// notice. Call it from the handler of the user's action. Refused, returning `false`, while New is unavailable
/// ([`new_available`]), while a dialog or menu is open (a trigger inside one closes it first, as the quick switcher's
/// row does), or with no UI session. While a host change is in flight, the notice's Open launcher still brings a
/// waiting launcher back (its Launch is refused meanwhile, so that changes nothing), and New does not.
pub fn open_new(window: &mut Window, cx: &mut App) -> bool {
    open_with(None, window, cx)
}

/// [`open_new`] with the session name set to `name`, as the quick switcher's "New session named X" row opens it
/// (SPEC.md: "Picking it opens New with that name and the default launch target and directory (Creation), plus
/// remembered permission defaults"). The name is a hand edit, as if typed into the Name field. A refused launch waiting
/// behind its notice comes back as it was, keeping its own name, as the old switcher's row brought it back.
pub fn open_new_named(name: &str, window: &mut Window, cx: &mut App) -> bool {
    open_with(Some(name), window, cx)
}

/// [`open_new`] and [`open_new_named`]'s shared path.
fn open_with(name: Option<&str>, window: &mut Window, cx: &mut App) -> bool {
    if layers::focus_layer_open(window, cx) || !new_available(cx) {
        return false;
    }
    if launch::open_waiting(window, cx) {
        return true;
    }
    let Some(store) = session::current_store(cx) else {
        return false;
    };
    let mut draft = new_draft(
        store.read(cx).collections(),
        local_state::selected(cx).as_deref(),
    );
    if let Some(name) = name {
        draft.edit_name(name.to_owned());
    }
    let mark = Mark::take(cx);
    open(draft, &mark, window, cx).is_some()
}

/// What the plain Replace YOLO question asks about; its owner holds the question as `prompt::Inline<YoloAsk>`
/// ([`ask_yolo_for_replace`]).
pub use super::view::yolo_question::YoloAsk;

/// Ask the YOLO question for a plain Replace the helm refused because the source's host asks before YOLO launches (the
/// refusal's `HelmMarker::YoloConfirmation`), for the sidebar to draw inline where the user answered Replace: at the
/// row, in the session header, or on the interrupted card (SPEC.md, "Creation": the confirmation is "shown with the
/// control or surface that started the launch").
///
/// `host` is the source's host and `host_name` its name as the rest of the app shows it
/// (`hosts::identity::display_name`), `launch` its stored launch (which says why it is YOLO), and `mark` the mark taken
/// when the user answered Replace: the question takes focus, on Cancel, only if the user has not moved on since
/// (SPEC.md: "a plain Replace's question, which arrives after its reply, takes focus only if the user has not moved on
/// since answering Replace"). There is no guarded answer: plain Replace keeps the stored launch.
///
/// `retry` sends the Replace again with `confirm_yolo` set, the caller's own command under its own key, as the
/// sidebar's Replace flow owns its launch and its retry. It runs at most once: for the one-off answer, or for the
/// permanent answer only after that answer's host write (the host settings' own "Start YOLO sessions without asking")
/// has succeeded; a failed write runs nothing and keeps the question up with the reason. It is handed the mark of the
/// answer's press, which the caller combines with its own operation's moved-on flag for what the Replace's reply may
/// open: the permanent answer's retry runs from the write's reply, after any press the user made meanwhile.
///
/// Drawing is the caller's: draw what this returns as a child (`.child(question.clone())`) from the next frame on,
/// where it is visible, scrolling its row, header or card into view (SPEC.md: "scrolled into view"); on the page, focus
/// moves to Cancel at once, and a question not drawn in the next frame loses it. Once answered it draws nothing. While
/// `question.in_flight(cx)` the permanent answer's write is out: if the caller stops drawing it meanwhile (the row
/// scrolled away, the selection changed), the write still runs on and its success still calls `retry`, which must cope
/// with the source having moved on. A reset drops the write's reply, leaving the question in flight for good, so drop
/// it with the UI session.
pub fn ask_yolo_for_replace(
    host: i64,
    host_name: HostText,
    launch: &SessionLaunch,
    mark: &Mark,
    retry: impl Fn(Mark, &mut Window, &mut App) + 'static,
    window: &mut Window,
    cx: &mut App,
) -> prompt::Inline<YoloAsk> {
    let ask = YoloAsk::for_stored_launch(host, host_name, launch);
    yolo_question::ask_question(&ask, None, replace_owner(Rc::new(retry)), mark, window, cx)
}

/// New's draft from what the store holds and the selected session `selected`: the open session's host and folder when
/// its host row still names the install the session was listed on, else the helm's own host and `~`; the remembered
/// permission. Prefilled values are seeds, not edits: nothing is marked touched.
fn new_draft(collections: &Collections, selected: Option<&str>) -> LaunchDraft {
    let host = open_host(collections, selected).or_else(|| {
        collections
            .ordered::<HostView>()
            .into_iter()
            .find(|host| HostKind::of(host) == HostKind::Local)
            .map(|host| host.id)
    });
    let folder = host.map_or_else(
        || "~".to_owned(),
        |host| default_folder(collections, selected, host),
    );
    let remembered = collections
        .get::<Preferences>(PREFERENCES_KEY)
        .and_then(|preferences| {
            remembered_permission(preferences.remembered_permissions.as_deref())
        });
    LaunchDraft::new(host, folder, remembered)
}

/// The host New starts on for the open session `selected`: its host row, when that row still names the install the
/// session was listed on and is not a host a sandbox or container owns. `None` without such a session.
fn open_host(collections: &Collections, selected: Option<&str>) -> Option<i64> {
    let row = collections.get::<SessionRow>(selected?)?;
    collections
        .ordered::<HostView>()
        .into_iter()
        .find(|host| same_install(row, host) && !host.is_session_owned)
        .map(|host| host.id)
}

/// The folder a draft seeds for `host` (SPEC.md, "Working directory"): "the selected session's directory when that
/// session is on the target host's installation", else `~`, "which each host expands to its own home". New's prefill
/// and the host picker's re-seed of an untouched folder both take it, so picking another host and then the open
/// session's again brings its folder back.
pub(super) fn default_folder(
    collections: &Collections,
    selected: Option<&str>,
    host: i64,
) -> String {
    let open = selected.and_then(|key| collections.get::<SessionRow>(key));
    let on_host = open.filter(|row| {
        collections
            .ordered::<HostView>()
            .into_iter()
            .any(|view| view.id == host && same_install(row, view))
    });
    on_host.map_or_else(|| "~".to_owned(), |row| row.info.cwd.clone())
}

/// Whether `host` is still the install session `row` was listed on (SPEC.md, "Host": "The comparison is by the
/// registry's recorded install identity, so it protects exactly the installs that have one: a host that has never
/// identified itself to the registry is compared by row id alone"). The identities must be equal, absent on both sides
/// included, as in the old app's comparison; a row retargeted onto another install since then does not match, so New
/// falls back to the helm's own host rather than aiming at the successor. A host whose reported identity contradicts
/// its recorded one is not that install either, whatever the registry still records.
fn same_install(row: &SessionRow, host: &HostView) -> bool {
    row.host == host.id
        && row.host_identity == host.identity
        && !matches!(host.state, HostStateView::IdentityMismatch { .. })
}
