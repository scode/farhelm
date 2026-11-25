//! The launcher's entry points: the ordinary public functions other surfaces call to open New (the session list's New
//! button, the quick switcher's "New session named X" row, Cmd+N), Clone and Replace with (the row menu and the session
//! header), Restart with (the header's Restart menu, with [`restart_with_unavailable`] for its greyed entry), what
//! those surfaces need to draw their triggers, the YOLO question the sidebar's plain Replace asks
//! ([`ask_yolo_for_replace`]), and what a quick launch needs: New's prefill as a draft, which it applies templates to
//! before deciding whether to show it ([`new_prefill`]), New opened on a draft with a banner or a refusal
//! ([`open_with`]), and the open launcher, whose draft a shortcut applies to ([`open_launcher`]).
//!
//! SPEC.md, "Creation" ("Working directory", "Host"), "Lifecycle operations" (Clone), "Desktop window chrome" (Cmd+N)
//! and the quick switcher's paragraph. Ported from the old app's New handling in `list/view.rs` at reference commit
//! `24bc0580` (`effective_create_host`, the New button's guards and its tooltip) and its Clone prefill in
//! `list/create_form.rs` (`resolve_clone_host`).
//!
//! New is refused while a launch is pending, and while a quick launch is being prepared out of sight before it starts
//! (`view::prepare_unseen`), which is a launch the user has asked for: the old New button was disabled while the
//! page's one operation token was held, and SPEC.md keeps "a launch is pending" among what disables it ("Cmd+N ... unless a modal is open or New is
//! disabled"). It is refused too while one of this window's host-registry changes is in flight (SPEC.md: "Launches and
//! changes to the host registry still exclude each other"; `crate::hosts::host_change_in_flight` says which work
//! counts), as the old token's host changes disabled it. A refused launch waiting behind its notice is different: New brings that
//! launcher back, with its draft and refusal, rather than starting a fresh one, as the old app did (its New revealed a
//! returned launcher). SPEC.md leaves open whether that notice keeps New refused until it is opened; bringing the draft
//! back answers it without refusing anything.
//!
//! New's prefill takes the open session's host and folder, else the helm's own host with `~` standing in until that
//! host's launch history names its folder; the host picker re-seeds an untouched folder from the same per-host default
//! ([`default_folder`]). An open session on a host that a sandbox or container owns lends New no host, since such a
//! host is no place for an unrelated session (the host picker leaves it out too): New's prefill takes the helm's own
//! host, and New opened by the user ([`open_new`], [`open_new_named`]) preselects, once as it opens, the recipe and
//! place of that session's environment for a fresh one (SPEC.md, "Sandbox sessions"), including when the user picks
//! the quick switcher's New row, as the old launcher did for
//! New opened without a prefill. A quick launch, which builds on [`new_prefill`], keeps its template's choices and
//! New's defaults instead: preselecting there would send a template that names no placement into a fresh container
//! beside a container session, its folder dropped for the container's home (decided by the launcher area on
//! 2026-11-16).
//!
//! The open session is read from the store, which holds it even while the sidebar's filter or host choice leaves it
//! unlisted (the window's session query names the open session, and the helm keeps it in what it pushes), so New
//! follows a hidden open session too, as the old app's "New inherits a filtered selected destination" case did.

use farhelm_helm_client::Collections;
use farhelm_proto::ui_channel::{
    HostStateView, HostView, PREFERENCES_KEY, Preferences, SessionRow,
};
use std::rc::Rc;

use farhelm_proto::{
    LaunchHarness, LaunchTarget, RETIRED_CONTAINER_REASON, ReadinessWording, RestartOffer,
    SessionInfo, SessionLaunch,
};
use gpui::{App, Entity, Window};

use super::draft::{FolderDefault, Install, LaunchDraft, harness_label, remembered_permission};
use super::launch;
use super::view::yolo_question::{self, GuardedAction, replace_owner};
use super::view::{Launcher, Note, Shown, open, open_noted};
use crate::focus::Mark;
use crate::host_text::HostText;
use crate::hosts::offers::HostKind;
use crate::prompt;
use crate::session_list::restart;
use crate::{layers, local_state, session};

/// New's hover text, for the button that opens it: it names the shortcut in the Mac app only, since the Linux app has
/// none (SPEC.md, "Desktop window chrome"). The old button's tooltip, word for word.
pub const NEW_SESSION_HELP: &str = if cfg!(target_os = "macos") {
    "New session: start an agent or a command on any host (⌘N)"
} else {
    "New session: start an agent or a command on any host"
};

/// Whether New can open now: not while a launch is pending or a quick launch is being prepared, nor while one of this
/// window's host-registry changes is in flight, when the old New button was disabled too (its page-wide operation token
/// covered launches and host changes, though not a new host's probe, which the migration's orchestrator decided counts
/// here). A surface drawing a New trigger draws it disabled while this is `false`, reading it while it draws: the
/// launch slot and the operation store both notify when what it answers from changes.
pub fn new_available(cx: &mut App) -> bool {
    !launch::busy(cx) && !crate::hosts::host_change_in_flight(cx)
}

/// Whether a quick launch may start now: as [`new_available`], and not while a refused launch waits behind its notice
/// either. That launcher is hidden, and SPEC.md has a hidden launcher take a quick launch's chord without applying it
/// ("A hidden quick-launch launcher still consumes the chord without applying it"); the old app's shortcuts did nothing
/// while its returned launcher stayed mounted behind the notice. New differs because it can bring that launcher back,
/// which a shortcut that starts a launch unseen cannot.
pub fn quick_launch_available(cx: &mut App) -> bool {
    !launch::holding(cx) && !crate::hosts::host_change_in_flight(cx)
}

/// Open New in `window` with New's prefill and, beside an open environment session, that environment's recipe and
/// place (see the module docs), or bring back a refused launch waiting behind its notice. Call it from the handler of
/// the user's action. Refused, returning `false`, while New is unavailable ([`new_available`]), while a dialog or menu
/// is open (a trigger inside one closes it first, as the quick switcher's row does), or with no UI session. While a
/// host change is in flight, the notice's Open launcher still brings a waiting launcher back (its Launch is refused
/// meanwhile, so that changes nothing), and New does not.
pub fn open_new(window: &mut Window, cx: &mut App) -> bool {
    new_beside_open_session(cx).is_some_and(|draft| open_with(draft, None, window, cx))
}

/// [`open_new`] with the session name set to `name`, as the quick switcher's "New session named X" row opens it
/// (SPEC.md: "Picking it opens New with that name and the default launch target and directory (Creation), plus
/// remembered permission defaults"). The name is a hand edit, as if typed into the Name field. A refused launch waiting
/// behind its notice comes back as it was, keeping its own name, as the old switcher's row brought it back.
pub fn open_new_named(name: &str, window: &mut Window, cx: &mut App) -> bool {
    new_beside_open_session(cx).is_some_and(|mut draft| {
        draft.edit_name(name.to_owned());
        open_with(draft, None, window, cx)
    })
}

/// The draft New opens on when the user opens it: [`new_prefill`], seeded with the environment the open session runs
/// in, if it runs in one ([`environment_of`]), so New starts a fresh environment like it. Only New's own openings take
/// this; a quick launch starts from [`new_prefill`] alone (see the module docs). `None` without a UI session.
fn new_beside_open_session(cx: &App) -> Option<LaunchDraft> {
    let mut draft = new_prefill(cx)?;
    let store = session::current_store(cx)?;
    let collections = store.read(cx).collections();
    let open = local_state::selected(cx).and_then(|key| collections.get::<SessionRow>(&key));
    if let Some(target) = open.and_then(|row| environment_of(collections, row)) {
        draft.seed_target(target);
    }
    Some(draft)
}

/// Open New on `draft`, saying `note` until the draft is next edited (a banner in its notices region, a refusal above its
/// search box): the one path every way of
/// opening New takes ([`open_new`], [`open_new_named`], and a quick launch that did not start directly, which opens it
/// with its templates applied and a banner or a refusal). Call it from the handler of the user's action. Refused,
/// returning `false`, as [`open_new`] is: while New is unavailable ([`new_available`]), while a dialog or menu is
/// open, or with no UI session; and a refused launch waiting behind its notice comes back instead, as it was, so a
/// launch the user has not seen the end of is never replaced by a fresh draft.
pub fn open_with(
    draft: LaunchDraft,
    note: Option<Note>,
    window: &mut Window,
    cx: &mut App,
) -> bool {
    if layers::focus_layer_open(window, cx) || !new_available(cx) {
        return false;
    }
    if launch::open_waiting(window, cx) {
        return true;
    }
    let mark = Mark::take(cx);
    // Through the launcher's own `open`; a quick launch's banner or refusal, set before the first frame, needs the
    // noted form.
    match note {
        None => open(draft, &mark, window, cx),
        Some(note) => open_noted(draft, None, Some(note), &mark, window, cx),
    }
    .is_some()
}

/// The launcher, when it is the dialog holding the keyboard in `window` (nothing over it but lists that never take
/// focus): template shortcuts and sequences apply to its draft then, with [`Launcher::apply_batch`], instead of
/// launching (SPEC.md, "Quick launch": "While the session launcher is the only visible dialog, the same chord applies
/// its template to that draft and leaves Launch to the user"). `None` while it is closed, hidden for a launch, or
/// covered.
///
/// It answers from the launcher-owned slot `show` sets each time a launcher is shown, never cleared by hand: a launcher
/// closes in several ways (Cancel, Escape through the layer manager, a submit, a reset closing every layer), and in each
/// its container leaves the layer stack, which this checks every time. The same check rules out a launcher a menu
/// covers.
pub(crate) fn open_launcher(window: &Window, cx: &App) -> Option<Entity<Launcher>> {
    let shown = cx.try_global::<Shown>()?;
    if layers::top_container(window, cx).as_ref() != Some(&shown.container) {
        return None;
    }
    shown.launcher.upgrade()
}

/// New's ordinary host and folder defaults for a quick launch. The template can set an environment explicitly,
/// while the open environment seeds a fresh recipe only when the user opens New. See SPEC.md, "Quick launch",
/// and `crate::templates::dispatch` for applying the template and dispatching. Read from the store alone, so
/// it is ready at once. `None` without a UI session. Unlike [`open_new`] it never brings back a refused launch waiting
/// behind its notice and checks nothing about whether New may open; the caller decides that.
pub fn new_prefill(cx: &App) -> Option<LaunchDraft> {
    let store = session::current_store(cx)?;
    Some(new_draft(
        store.read(cx).collections(),
        local_state::selected(cx).as_deref(),
    ))
}

/// Open Clone of the session `session` (its key) in `window`: an ordinary, editable create prefilled from the
/// session's host, folder, title and stored launch, verbatim, with the initial prompt and the conversation selector
/// empty (SPEC.md, "Clone"). Call it from the handler of the user's action (the row menu's or the session header's
/// Clone). Refused, returning `false`, as [`open_new`] is: while New is unavailable (a surface drawing Clone draws it
/// disabled then, reading [`new_available`]), while a dialog or menu is open, with no UI session, and also when the
/// store no longer holds the session (a session the list's filter hides is still held while it is open).
///
/// The host is the session's own when its host row still names the install the session was listed on and is not a
/// host a sandbox or container owns. A session in a sandbox or container is cloned into a fresh environment made from
/// the same recipe at the same place, never onto its disposable host and never with a copy of its disk (SPEC.md,
/// "Sandbox sessions": "Clone targets the recipe for a fresh sandbox with the source's launch and repository intent";
/// "Container sessions": "Clone creates a new container from the recipe at its selected place"). Its folder is the new
/// environment's home, its launch and any managed checkout's repository are the source's, and New's host (the open
/// session's, else the helm's own) waits behind Existing host. The environment is found by the source's host id, as
/// the old launcher found it ([`environment_of`]); a recipe or place gone since stays chosen, shown unavailable with its
/// reason ([`LaunchDraft::seed_target`]).
///
/// Otherwise the host is not carried over: the draft takes New's host instead, and a note under Host says why, as the
/// old launcher did once the registry could not confirm the source's install ([`HOST_NOT_CARRIED`]). That covers a
/// session-owned host that names no environment to carry too (decided by the launcher area on 2026-11-16). The folder
/// stays the session's then, as SPEC.md has it ("Other sessions keep their working directory"); like any prefilled
/// folder it is untouched, so picking another host re-seeds it as in New.
///
/// A refused launch waiting behind its notice comes back instead, with its draft and refusal, as New brings it back:
/// that refusal arrived after the user had moved on, so they have not seen it yet, and replacing its draft would lose
/// it unseen. Clone of the session works again once that launcher is closed, or once its launch has had its reply.
///
/// A session in a managed checkout is cloned onto a new managed checkout of its repository, named `<title>-clone`;
/// choosing Folder brings back its folder and title ([`LaunchDraft::clone_of`]).
pub fn open_clone(session: &str, window: &mut Window, cx: &mut App) -> bool {
    if layers::focus_layer_open(window, cx) || !new_available(cx) {
        return false;
    }
    if launch::open_waiting(window, cx) {
        return true;
    }
    let Some(store) = session::current_store(cx) else {
        return false;
    };
    let collections = store.read(cx).collections();
    let Some(row) = collections.get::<SessionRow>(session) else {
        return false;
    };
    let source_host = collections
        .ordered::<HostView>()
        .into_iter()
        .find(|host| same_install(row, host));
    let (host, environment, note) = match source_host {
        Some(host) if !host.is_session_owned => (Some(host.id), None, None),
        _ => {
            let selected = local_state::selected(cx);
            let fallback =
                open_host(collections, selected.as_deref()).or_else(|| local_host(collections));
            match environment_of(collections, row) {
                Some(target) => (fallback, Some(target), None),
                None => (fallback, None, Some(HOST_NOT_CARRIED)),
            }
        }
    };
    let mut draft = LaunchDraft::clone_of(&row.info, host);
    if let Some(target) = environment {
        draft.seed_target(target);
    }
    let mark = Mark::take(cx);
    open_noted(draft, note, None, &mark, window, cx).is_some()
}

/// Open Replace with of the session `session` (its key) in `window`: the same launcher Clone opens, prefilled from the
/// session's host, folder (its existing directory, even for a managed checkout), title and stored launch, whose launch
/// creates the new session and then deletes the source, with plain Replace's contract (SPEC.md, "Replace with"). Call
/// it from the handler of the user's action (the row menu's or the session header's "Replace with…"). Refused, returning
/// `false`, exactly as [`open_clone`] is, and it likewise brings back a refused launch waiting behind its notice
/// instead.
///
/// The host is the source's own row, held: the helm refuses a Replace with on another host, Clone being the way to move
/// hosts, so there is no fallback and no note. While the source has anything alive the launcher shows Replace's warning
/// beside its main action, following the source's state while it stays open, and a press carries the precondition
/// matching what it showed. The replacement opens only while its source is still the open session, as plain Replace's
/// does, and the source's row and header show the request while it waits, as they show plain Replace's.
pub fn open_replace_with(session: &str, window: &mut Window, cx: &mut App) -> bool {
    if layers::focus_layer_open(window, cx) || !new_available(cx) {
        return false;
    }
    if launch::open_waiting(window, cx) {
        return true;
    }
    let Some(store) = session::current_store(cx) else {
        return false;
    };
    let Some(row) = store.read(cx).collections().get::<SessionRow>(session) else {
        return false;
    };
    if crate::session_list::environment::of_key(session, cx)
        .is_some_and(|host| !host.permits_launch_preparation())
    {
        return false;
    }
    let draft = LaunchDraft::replace_with(session, &row.info, Some(row.host));
    let mark = Mark::take(cx);
    open(draft, &mark, window, cx).is_some()
}

/// Open Restart with of the session `session` (its key) in `window`: the launcher in restart mode, showing the session's
/// current settings with its launch kind, agent type, host, folder, name and initial prompt held, whose main action
/// restarts the session with the edited settings, resuming its own or a named conversation (SPEC.md, "Restart with").
/// Call it from the handler of the user's action (the session header's Restart menu). Refused, returning `false`, as
/// [`open_clone`] is, and also while [`restart_with_unavailable`] names a reason; like New, it brings back a refused
/// launch waiting behind its notice instead.
///
/// Its main action reads "Stop and restart" while the session's agent reads working, and a press is then the consent to
/// stop it; the session's row and header show the request while it waits, as for a plain Restart, and a success opens
/// nothing (the session keeps running where it is). Focus starts on Cancel.
pub fn open_restart_with(session: &str, window: &mut Window, cx: &mut App) -> bool {
    if layers::focus_layer_open(window, cx) || !new_available(cx) {
        return false;
    }
    if launch::open_waiting(window, cx) {
        return true;
    }
    let Some(store) = session::current_store(cx) else {
        return false;
    };
    let Some(row) = store.read(cx).collections().get::<SessionRow>(session) else {
        return false;
    };
    if restart_with_reason(row).is_some()
        || crate::session_list::environment::of_key(session, cx)
            .is_some_and(|host| !host.permits_launch_preparation())
    {
        return false;
    }
    let Some(draft) = LaunchDraft::restart_with(session, &row.info, Some(row.host)) else {
        return false;
    };
    let mark = Mark::take(cx);
    open(draft, &mark, window, cx).is_some()
}

/// Why Restart with is not offered for the session `session` (its key), for the header's Restart menu to draw its entry
/// greyed with this as its hover help and accessible description (SPEC.md, "Restart with": "visible but greyed out
/// otherwise, with a hover tooltip and accessible description explaining why"); `None` when it is offered. A session
/// the store no longer holds has none to offer. This answers only whether the session's launch can be restarted with
/// edits; while a launch is pending or a dialog is open, [`open_restart_with`] refuses as New does, which the menu shows
/// by [`new_available`] like its other launcher entries.
pub fn restart_with_unavailable(session: &str, cx: &App) -> Option<String> {
    let store = session::current_store(cx)?;
    let collections = store.read(cx).collections();
    let Some(row) = collections.get::<SessionRow>(session) else {
        return Some("This session is no longer listed".to_owned());
    };
    restart_with_reason(row)
}

/// Restart with's fallback wording when there is no conversation it can resume and no more specific explanation.
const NO_CAPTURED_CONVERSATION: &str = "No captured conversation to resume";

/// Name the declared agent in the shared refusal. Each launch kind determines when this refusal applies.
fn unsupported_resume_reason(harness: LaunchHarness) -> String {
    format!("{} sessions can't be resumed", harness_label(harness))
}

/// The old app's `restart_with_reason` (`session_view.rs` at the reference commit): a legacy launch is never changed
/// (Replace with is the way), an agent launch can be edited when its conversation can be resumed or it has a stored
/// resume command to take a named one, and a command launch when it declared an agent type and a resume command and its
/// conversation can be resumed. A retired container session supports Delete only.
pub(super) fn restart_with_reason(row: &SessionRow) -> Option<String> {
    let info = &row.info;
    let offer = info.restart_offer;
    if info.launch.is_retired_container() {
        return Some(RETIRED_CONTAINER_REASON.to_owned());
    }
    if info.launch.is_shell() {
        return Some(
            "This shell has no conversation to resume. Replace starts a fresh shell.".to_owned(),
        );
    }
    match &info.launch {
        SessionLaunch::Agent {
            selection, resume, ..
        } => {
            if offer == RestartOffer::Resume || resume.is_some() {
                return None;
            }
            // Reporting support and a stored resume command are separate facts: an older agent launch can gain
            // tracking without gaining a resume command, which needs recreating, not a claim about capture.
            if selection.harness.agent_kind().restart_readiness().is_none() {
                Some(unsupported_resume_reason(selection.harness))
            } else if offer == RestartOffer::NoResumeCommand {
                Some(
                    "This agent launch has no resume command, so it cannot be restarted; use Clone or Replace with"
                        .to_owned(),
                )
            } else if offer == RestartOffer::NotCaptured {
                Some(not_captured_reason(info))
            } else {
                Some(NO_CAPTURED_CONVERSATION.to_owned())
            }
        }
        SessionLaunch::Command(command) => {
            if command.agent.is_none() || command.resume.is_none() {
                return Some(
                    "This command launch has no resume command, so it cannot be restarted; use Replace with"
                        .to_owned(),
                );
            }
            match offer {
                RestartOffer::Resume => None,
                // A declared type Farhelm has no conversation reporting for can carry a resume command, and will never
                // have a conversation to fill it with.
                RestartOffer::NoConversationReporting => command.agent.map(unsupported_resume_reason),
                RestartOffer::NotCaptured => Some(not_captured_reason(info)),
                RestartOffer::NoResumeCommand => Some(NO_CAPTURED_CONVERSATION.to_owned()),
            }
        }
        SessionLaunch::Legacy { .. } => Some(
            "This session was created before launch kinds, so its launch cannot be changed; use Replace with"
                .to_owned(),
        ),
    }
}

/// Restart with's reason for an uncaptured conversation: the Restart control's per-agent fact in a short form, since
/// Restart with waits on the same conversation, ending with the same feedback invitation (the old
/// `not_captured_with_reason`).
fn not_captured_reason(info: &SessionInfo) -> String {
    if let Some(reason) = info
        .persistent_hook_setup
        .restart_blocker(info.agent_kind, ReadinessWording::ToTheUser)
    {
        return format!("{reason}.");
    }
    let ended = info.status.has_ended();
    let readiness = info
        .agent_kind
        .display_name()
        .zip(info.agent_kind.restart_readiness());
    let reason = match (readiness, ended) {
        (Some((name, readiness)), false) => format!(
            "For {name} sessions, Restart with becomes available {}",
            readiness.clause(ReadinessWording::ToTheUser)
        ),
        (None, false) => {
            "Restart with becomes available once Farhelm captures this session's conversation"
                .to_owned()
        }
        (Some((name, _)), true) => {
            format!("Farhelm never captured this {name} session's conversation")
        }
        (None, true) => "Farhelm never captured this session's conversation".to_owned(),
    };
    format!("{reason}.{}", restart::feedback_note(info, ended))
}

/// The note under Host when a Clone's source host no longer names the install the session ran on (retargeted onto
/// another machine, gone from the registry, or in an identity mismatch), or names a host a sandbox or container owns
/// with no environment to carry ([`environment_of`]): the old launcher's words, which it showed for an install
/// mismatch. They fit the last case poorly; the launcher area chose one note for every host not carried (2026-11-16).
pub(super) const HOST_NOT_CARRIED: &str = "The source session reports a different installation now, so its host was \
     not carried over here — check the host below";

/// The fresh environment a session's own environment stands for: the recipe and place of the sandbox or container that
/// owns the host `row` runs on (SPEC.md, "Sandbox sessions" and "Container sessions": Clone, and New beside it, make a
/// new one from them). Found by the host's registry id alone, with no installation check, as the old launcher found it
/// (kept by the launcher area on 2026-11-16). `None` when that host is not one an environment owns, or carries no
/// environment view to say which.
fn environment_of(collections: &Collections, row: &SessionRow) -> Option<LaunchTarget> {
    let host = collections.get::<HostView>(&HostView::key(row.host))?;
    if !host.is_session_owned {
        return None;
    }
    host.environment
        .as_ref()
        .map(|environment| environment.target.clone())
}

/// What the plain Replace YOLO question asks about; its owner holds the question as `prompt::Inline<YoloAsk>`
/// ([`ask_yolo_for_replace`]).
pub use super::view::yolo_question::YoloAsk;

/// The user can keep the saved launch or choose a newly composed Guarded replacement.
pub(crate) enum ReplaceAnswer {
    Yolo,
    Guarded,
}

/// Ask the YOLO question for a plain Replace the helm refused because the source's host asks before YOLO launches (the
/// refusal's `HelmMarker::YoloConfirmation`), for the sidebar to draw inline where the user answered Replace: at the
/// row, in the session header, or on the interrupted card (SPEC.md, "Creation": the confirmation is "shown with the
/// control or surface that started the launch").
///
/// `host` is the source's host and `host_name` its name as the rest of the app shows it
/// (`hosts::identity::display_name`), `launch` its stored launch (which says why it is YOLO), and `mark` the mark taken
/// when the user answered Replace: the question takes focus, on Cancel, only if the user has not moved on since
/// (SPEC.md: "a plain Replace's question, which arrives after its reply, takes focus only if the user has not moved on
/// since answering Replace"). A saved YOLO launch can offer a guarded replacement when its agent supports Guarded.
///
/// `retry` dispatches the chosen replacement under the caller's own key. It runs at most once: for Guarded, the one-off answer, or for the
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
pub(crate) fn ask_yolo_for_replace(
    host: i64,
    host_name: HostText,
    launch: &SessionLaunch,
    mark: &Mark,
    retry: impl Fn(ReplaceAnswer, Mark, &mut Window, &mut App) + 'static,
    window: &mut Window,
    cx: &mut App,
) -> prompt::Inline<YoloAsk> {
    let ask = YoloAsk::for_stored_launch(host, host_name, launch, cx);
    let retry = Rc::new(retry);
    let guarded = ask.guarded.then(|| {
        let retry = Rc::clone(&retry);
        GuardedAction::replace(move |mark, window, cx| {
            retry(ReplaceAnswer::Guarded, mark, window, cx)
        })
    });
    let owner = replace_owner(Rc::new(move |mark, window, cx| {
        retry(ReplaceAnswer::Yolo, mark, window, cx)
    }));
    yolo_question::ask_question(&ask, guarded, owner, mark, window, cx)
}

/// New's draft from what the store holds and the selected session `selected`: the open session's host and folder when
/// its host row still names the install the session was listed on, else the helm's own host and `~`; the remembered
/// permission. Prefilled values are seeds, not edits: nothing is marked touched.
fn new_draft(collections: &Collections, selected: Option<&str>) -> LaunchDraft {
    let host = open_host(collections, selected).or_else(|| local_host(collections));
    let inherited = host.and_then(|host| session_folder(collections, selected, host));
    let pending = host.is_some() && inherited.is_none();
    let folder = inherited.unwrap_or_else(|| "~".to_owned());
    let remembered = collections
        .get::<Preferences>(PREFERENCES_KEY)
        .and_then(|preferences| {
            remembered_permission(preferences.remembered_permissions.as_deref())
        });
    let mut draft = LaunchDraft::new(host, folder, remembered);
    // Without the open session's folder, `~` stands in until the host's history names its own default
    // (`LaunchDraft::folder_default`). The open session's folder is New's default for the installation it is on.
    if pending {
        draft.folder_default = FolderDefault::Pending;
    } else if let Some(view) = host.and_then(|id| {
        collections
            .ordered::<HostView>()
            .into_iter()
            .find(|view| view.id == id)
    }) {
        draft.folder_install = Some(Install::of(view));
    }
    draft
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

/// The helm's own host, New's and Clone's fallback target: "the helm's own host" in SPEC.md, "Host". `None` only
/// before the helm has reported it.
pub(super) fn local_host(collections: &Collections) -> Option<i64> {
    collections
        .ordered::<HostView>()
        .into_iter()
        .find(|host| HostKind::of(host) == HostKind::Local)
        .map(|host| host.id)
}

/// The folder a draft seeds for `host` (SPEC.md, "Working directory"): "the selected session's directory when that
/// session is on the target host's installation", else `~`, "which each host expands to its own home". New's prefill
/// and the host picker's re-seed of an untouched folder both take it, so picking another host and then the open
/// session's again brings its folder back; so does a quick launch's template that changes the host, which re-seeds the
/// folder the same way (`crate::templates::apply::Context::default_folder`).
///
/// The step between those two, the host's own default from its launch history, arrives later, from the launcher's
/// history read (`LaunchDraft::folder_default`); until it does, `~` stands in, and Launch waits for it.
pub(crate) fn default_folder(
    collections: &Collections,
    selected: Option<&str>,
    host: i64,
) -> String {
    session_folder(collections, selected, host).unwrap_or_else(|| "~".to_owned())
}

/// The selected session `selected`'s folder when that session is on `host`'s installation: the first choice of
/// [`default_folder`], which wins over the host's history without waiting for it.
pub(crate) fn session_folder(
    collections: &Collections,
    selected: Option<&str>,
    host: i64,
) -> Option<String> {
    let open = selected.and_then(|key| collections.get::<SessionRow>(key));
    open.filter(|row| {
        collections
            .ordered::<HostView>()
            .into_iter()
            .any(|view| view.id == host && same_install(row, view))
    })
    .map(|row| row.info.cwd.clone())
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
