//! Removing an SSH host from the registry: the host menu's Remove, its confirmation, and the request.
//!
//! SPEC.md, "Topology": "Removing a host from the registry merely forgets it — the supervisor and its sessions are
//! untouched and reappear on re-registration." Ported from the old app's `HostRemoveDialog` and `RemoveCopy`
//! (`hosts/settings_dialog.rs`) and the Hosts panel's `on_remove_start` and `on_remove_confirm` (`hosts.rs`), at
//! reference commit `24bc0580`. Here's the TLDR of its rules:
//!
//! - **Asked or not.** The menu's Remove ([`start`]) asks the confirmation unless "Remove hosts without asking" is on
//!   ([`asks_first`]); then the removal goes out at once, as the old remembered answer did.
//! - **The confirmation** ([`ask`]) is a question of the prompt, frozen on the host's id ([`Removal`]): "Remove host ·
//!   <name>", the host's phase and build, then a body drawn at the old dialog's sizes ([`RemoveBody`]: the destination
//!   and the old explanation), and the footer note naming the gear as the way to undo the permanent answer. Three
//!   answers in the old order: Cancel, "Remove, don't ask again" and Remove, where focus starts. When the host turned
//!   busy between the menu and the question, Remove is unavailable and focus starts on Cancel (SPEC.md, "Opening and
//!   closing"), and the permanent answer is left out, with the footer note that names it, since it would be a second
//!   way to remove; the old dialog disabled both. The permanent answer also turns the preference on, under the
//!   preference row's key, but only once its removal has been sent: a removal refused before anything is sent (below)
//!   changes no preference either. The question closes when the registry no longer lists the host. Cancel returns
//!   focus to what held it when the question opened: the row's menu toggle, which the menu gave focus back to as it
//!   closed.
//! - **The request** ([`submit`]) is `DELETE /api/hosts/{id}`, the operation store's work under the host's key, so it
//!   goes on after every view involved has closed. A success needs nothing here: the pushed host list drops the row, and
//!   focus, back on the row's toggle since the question closed, recovers as from any control that goes (SPEC.md, "Where
//!   focus rests"). A refusal is the store's record under the host's key, which the Hosts panel shows under the host's
//!   row, opening it. Nothing a result does opens a dialog or moves focus, so a result that arrives after the user moved
//!   on needs no check of its own; the question itself opens only through the mark taken at the menu's Remove.
//! - **Not started.** A Remove refused before anything is sent says why on the Hosts panel's line, naming the host,
//!   until the next Remove or a reset ([`panel_lines`]): the host's key already has work in flight (the store's
//!   refusal), or a session launch is in flight ([`launch_in_flight`]). The second is the migration's rule, decided by
//!   its orchestrator on 2026-11-14 after the old app refused a host change during a launch and the reverse: each side
//!   refuses its own action while the other's work is in flight in the operation store, with a reason, and no shared
//!   lock.
//! - **A refused permanent answer.** The removal goes ahead whether or not its preference save succeeds, and the
//!   question has closed, so a refusal of that save shows on the panel's line too, "Remove hosts without asking:" and
//!   the helm's words, or Settings' words for a press refused that way ([`Refused::WORDS`]) when the operation store
//!   refused it because another preference save was in flight, until the next permanent answer, a reset, or the
//!   preference turning on.
//!
//! NOTE: Three of the recorded deviations (`docs/gpui/parity/settings-and-hosts.md`). The old dialog stayed open
//! through its request and showed a refusal inside it; the prompt keeps a question open through its action only for an
//! inline question's extra answer, so here the question closes on Remove and a refusal shows under the row. With the
//! preference on, the old app opened a short refusal dialog, which is not ported yet; that refusal shows under the row
//! too. And the old header drew the phase with its colored dot; the prompt's summary is a line of text, so the phase
//! words and build are drawn in the summary's ink.
//!
//! A Tensorlake or container host's removal deletes its machine and is a different command (Delete sandbox), which a
//! later piece of the port brings; the menu offers no Remove on those hosts ([`super::offers::HostKind`]).

#[cfg(test)]
mod tests;

use farhelm_helm_client::{ApiPath, Method, PathError};
use farhelm_proto::ui_channel::{HostView, PREFERENCES_KEY, Preferences};
use gpui::{
    App, AppContext as _, Context, EntityId, Global, IntoElement, ParentElement as _, Render,
    SharedString, Styled as _, Subscription, Window, div, px,
};

use super::identity::{display_name, host_mark, phase};
use super::offers;
use super::setup::UNDO_NOTE;
use crate::assets;
use crate::components::{HoverHelp, Tier};
use crate::focus::Mark;
use crate::host_text::{self, HostSentence, HostText};
use crate::layers::{LayerId, Subject};
use crate::operations::{Command, NotStarted, OpKey, Outcome, Work};
use crate::prompt::{self, Answer, Question};
use crate::session;
use crate::settings::Refused;
use crate::theme;

/// The host's mark in the confirmation's header tile, as the host settings dialog draws it.
const TILE_MARK: f32 = 24.;

/// The body's text size: the old dialog's body inherited the page's 16 pixels, larger than the dialog frame's 12.
const BODY_TEXT: f32 = 16.;

/// `.host-remove-explanation`'s `line-height: 1.45`, at [`BODY_TEXT`].
const EXPLANATION_LINE: f32 = 23.2;

/// The destination line's own margin above and below: the old `<p>`'s browser default of `1em`, which the stylesheet
/// left in place for `.host-dialog-destination` (the explanation's was zeroed).
const DESTINATION_MARGIN: f32 = 16.;

/// `.host-settings-body`'s `gap: 20px` between the destination and the explanation.
const BODY_GAP: f32 = 20.;

/// The old confirmation's explanation of what removing does.
const EXPLANATION: &str = "Farhelm will forget this host. Its supervisor and sessions keep running, and adding the \
                           destination again finds them.";

/// The menu item's and the Remove answer's hover help, the old Remove answer's.
pub(super) const REMOVE_HELP: &str =
    "Remove: forget this host; its supervisor and sessions keep running";

/// What the confirmation acts on, frozen when it opens: the host by registry id, its name as the question showed it,
/// which a line about a removal that was not started names it by, and its destination, which the body shows.
#[derive(Clone)]
struct Removal {
    host: i64,
    name: HostText,
    destination: HostText,
}

/// The host menu's Remove for `host`, pressed with `mark` taken in its handler: ask the confirmation, or with "Remove
/// hosts without asking" on, send the removal at once. Refused with the panel line's reason while a session launch is in
/// flight; nothing happens when the window is inert or the registry no longer lists the host.
pub(super) fn start(host: i64, mark: &Mark, window: &mut Window, cx: &mut App) {
    let Some(view) = listed(host, cx) else {
        return;
    };
    let removal = Removal::of(&view);
    // A line from an earlier Remove belongs to that attempt; this one says its own.
    show(cx, |lines| lines.not_started = None);
    if launch_in_flight(cx) {
        not_started(&removal, Refusal::LaunchInFlight, cx);
    } else if asks_first(cx) {
        let _ = ask(host, mark, window, cx);
    } else {
        // TODO: with the preference on, the old app showed a refusal of this removal in the short host refusal dialog
        // ("Remove host · <name>", Close), opened through `mark`; it shows under the row until that dialog is ported
        // with Update's refusals.
        submit(&removal, cx);
    }
}

impl Removal {
    /// The removal of `view` as the registry lists it now.
    fn of(view: &HostView) -> Removal {
        Removal {
            host: view.id,
            name: display_name(view),
            // The destination in full, since the title may shorten the name (SPEC.md: "A shortened heading retains the
            // full destination in the body").
            destination: HostText::from_host(view.destination.as_deref().unwrap_or(&view.name)),
        }
    }
}

/// Host `host` as the registry lists it now, or `None` when the window is inert or it is not listed.
fn listed(host: i64, cx: &App) -> Option<HostView> {
    let store = session::current_store(cx)?;
    let collections = store.read(cx).collections();
    collections.get::<HostView>(&HostView::key(host)).cloned()
}

/// Whether a Remove asks first: the shared "Remove hosts without asking" preference is off or the helm stored no
/// choice.
fn asks_first(cx: &App) -> bool {
    let skips = session::current_store(cx).is_some_and(|store| {
        store
            .read(cx)
            .collections()
            .get::<Preferences>(PREFERENCES_KEY)
            .and_then(|preferences| preferences.skip_host_remove_confirmation)
            == Some(true)
    });
    !skips
}

/// Whether a session launch is in flight anywhere in the operation store, which refuses a Remove (the module docs).
fn launch_in_flight(cx: &App) -> bool {
    session::current_operations(cx)
        .is_some_and(|operations| operations.read(cx).any_in_flight(creates_a_session))
}

/// Whether `work` creates a session on a host: what "a launch in flight" means for the rule that a host's removal waits
/// for one: the launcher's New and Clone, and a session's Restart and Replace, each of which creates a session (decided
/// by the migration's orchestrator on 2026-11-14). Exhaustive, so a new kind of work decides.
fn creates_a_session(work: Work) -> bool {
    match work {
        Work::Launch | Work::Restart | Work::Replace => true,
        Work::Rename
        | Work::Delete(_)
        | Work::Save
        | Work::Stop
        | Work::Seen
        | Work::Probe
        | Work::Setup
        | Work::TunnelOpen
        | Work::TunnelClose
        | Work::RemoveHost => false,
    }
}

/// Ask the removal confirmation for `host` over everything else in `window` (see the module docs), unless the user has
/// moved on since `mark`, the window is inert, or the registry does not list the host. Returns the question's layer.
/// Visible to the crate for the screenshot golden.
pub(crate) fn ask(host: i64, mark: &Mark, window: &mut Window, cx: &mut App) -> Option<LayerId> {
    let store = session::current_store(cx)?;
    let key = HostView::key(host);
    let (view, enabled) = {
        let collections = store.read(cx).collections();
        let view = collections.get::<HostView>(&key)?.clone();
        let in_flight = session::current_operations(cx)
            .is_some_and(|operations| operations.read(cx).in_flight(&OpKey::host(&key)).is_some());
        let enabled = offers::offers(&view, collections.get(&key), in_flight).remove == Some(true);
        (view, enabled)
    };
    let removal = Removal::of(&view);
    let name = removal.name.clone();
    let title = HostSentence::new("host-remove-title")
        .text("Remove host · ")
        .host(&name);
    let mut summary = HostSentence::new("host-remove-summary").text(phase::words(&view.state));
    if let Some(build) = phase::reported_build(&view.state) {
        summary = summary
            .text(" · Farhelm ")
            .mono(&HostText::from_host(build));
    }
    let (icon, color) = (view.icon, view.color);
    let remove = Answer {
        label: "Remove".into(),
        help: HoverHelp::tooltip(REMOVE_HELP),
        tier: Tier::Danger,
    };
    let mut question = Question::new(title, remove, removal)
        .summary(summary)
        .body(|removal: &Removal, _, cx| {
            let body = RemoveBody {
                destination: removal.destination.clone(),
            };
            cx.new(|_| body)
        })
        .confirm_enabled(enabled)
        .cancel_answer(Answer {
            label: "Cancel".into(),
            help: HoverHelp::tooltip("Cancel: keep this host"),
            tier: Tier::Neutral,
        });
    // Only beside an available Remove: with Remove unavailable it would be the one way left to remove the host. The
    // footer note names this answer, so it goes with it.
    if enabled {
        question = question.note(UNDO_NOTE).alternative(
            Answer {
                label: "Remove, don't ask again".into(),
                help: HoverHelp::tooltip("Remove it, and remove hosts without asking from now on"),
                tier: Tier::DangerOutline,
            },
            |removal, _, cx| {
                // The preference follows the removal it answers: a removal refused before it was sent changes nothing.
                if submit(&removal, cx) {
                    stop_asking(cx);
                }
            },
        );
    }
    let question = question
        .tile(move |_, cx| host_mark(icon, color, px(TILE_MARK), theme::palette(cx)))
        // SPEC.md, "Opening and closing": the dialogs that share the host and Settings installer keep focus on the
        // control in use when their empty space is clicked.
        .keeps_focus_on_empty_click();
    let subject = Subject::when(store, move |store, _| super::settings::gone(store, &key));
    prompt::ask(
        question,
        Some(subject),
        mark,
        |removal, _, cx| {
            submit(&removal, cx);
        },
        window,
        cx,
    )
}

/// `DELETE /api/hosts/{id}` for host `host`: the helm forgets the entry and answers with an empty object, or refuses
/// (the helm's own machine, an unknown id, or a host whose setup, update or uninstall is running).
fn remove_command(host: i64) -> Result<Command, PathError> {
    Ok(Command::new(
        Work::RemoveHost,
        Method::Delete,
        ApiPath::api(&["hosts", &HostView::key(host)])?,
        None,
        "DELETE /api/hosts/[host]",
    ))
}

/// Send `removal` under its host's key as the operation store's work, unless a session launch is in flight or the key
/// is busy, which the panel's line then says (the module docs). Returns whether the removal was sent, which the
/// permanent answer's preference waits on. The store records a refusal under the host's key, where the Hosts panel
/// shows it, so the completion has nothing to do.
fn submit(removal: &Removal, cx: &mut App) -> bool {
    if launch_in_flight(cx) {
        not_started(removal, Refusal::LaunchInFlight, cx);
        return false;
    }
    let Some(operations) = session::current_operations(cx) else {
        return false;
    };
    let command = match remove_command(removal.host) {
        Ok(command) => command,
        Err(error) => {
            tracing::debug!(?error, "host removal not built");
            return false;
        }
    };
    let key = OpKey::host(&HostView::key(removal.host));
    let started = operations.update(cx, |operations, cx| {
        operations.start(&key, command, cx, |_, _| {})
    });
    match started {
        Ok(()) => {
            show(cx, |lines| lines.not_started = None);
            true
        }
        Err(NotStarted::Busy(_)) => {
            not_started(removal, Refusal::Busy, cx);
            false
        }
        // A reset retired the store: the generic connection-lost notice is the report.
        Err(not_started) => {
            tracing::debug!(?not_started, "host removal not sent");
            false
        }
    }
}

/// Turn on "Remove hosts without asking" for the permanent answer: only its field of the preference row, under the
/// row's key, as Settings' switch saves it. Not retried. A refusal shows on the Hosts panel's line (the module docs),
/// and so does the operation store's refusal to start it while another save is in flight under the preference row's
/// key: Settings' switches, the template editor, ntfy's form, the checkout field and the setup confirmation's own
/// permanent answer all save there, and their work outlives the views that started it, so one can still be in flight
/// while this question is open.
fn stop_asking(cx: &mut App) {
    let Some(operations) = session::current_operations(cx) else {
        return;
    };
    let command = match crate::settings::skip_host_remove_command() {
        Ok(command) => command,
        Err(error) => {
            tracing::debug!(?error, "the permanent answer's preference not built");
            return;
        }
    };
    let started = operations.update(cx, |operations, cx| {
        operations.start(&OpKey::preferences(), command, cx, |settled, cx| {
            if let Outcome::Refused(refusal) = &settled.outcome {
                let line = stop_asking_line().host(&refusal.words);
                let store = current_store_id(cx);
                show(cx, |lines| {
                    lines.stop_asking = store.map(|store| (store, line))
                });
            }
        })
    });
    match started {
        Ok(()) => show(cx, |lines| lines.stop_asking = None),
        Err(NotStarted::Busy(_)) => {
            let line = stop_asking_line().text(Refused::WORDS);
            let store = current_store_id(cx);
            show(cx, |lines| {
                lines.stop_asking = store.map(|store| (store, line))
            });
        }
        // A reset retired the store: the generic connection-lost notice is the report.
        Err(not_started) => {
            tracing::debug!(?not_started, "the permanent answer's preference not sent")
        }
    }
}

/// The start of the panel's line about the permanent answer's preference, under its own id.
fn stop_asking_line() -> HostSentence {
    HostSentence::new("hosts-remove-stop-asking-error").text("Remove hosts without asking: ")
}

// ---- The body ------------------------------------------------------------------------------------------------------

/// The confirmation's body (`.host-settings-body` of the old `HostRemoveDialog`): the host's destination in the
/// monospace face, then what removing does, both at the old dialog's 16 pixels, which the prompt's lines (at the dialog
/// frame's 12) cannot give. Built once from the frozen [`Removal`], so it shows the destination the question was asked
/// about.
struct RemoveBody {
    destination: HostText,
}

impl Render for RemoveBody {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = theme::palette(cx);
        div()
            .flex()
            .flex_col()
            .gap(px(BODY_GAP))
            .text_size(px(BODY_TEXT))
            .child(
                // `.host-settings-value.host-dialog-destination`: the ordinary ink, wrapping anywhere.
                host_text::element(&self.destination)
                    .my(px(DESTINATION_MARGIN))
                    .font_family(assets::TERMINAL_FONT)
                    .text_color(theme::color(palette.fg_0)),
            )
            .child(
                // `.host-remove-explanation`: the quiet ink at a line height of 1.45.
                div()
                    .line_height(px(EXPLANATION_LINE))
                    .text_color(theme::color(palette.fg_1))
                    .child(EXPLANATION),
            )
    }
}

// ---- The Hosts panel's line ------------------------------------------------------------------------------------------

/// Why a Remove was not started.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Refusal {
    /// The host's key already has work in flight: the operation store refused the request.
    Busy,
    /// A session launch is in flight (the module docs).
    LaunchInFlight,
}

impl Refusal {
    /// What the line says after the host's name, in the old app's wording for an action refused while another runs
    /// ("<action>: not started, another operation is in progress; try again", `list/lifecycle.rs` at the reference
    /// commit).
    fn words(self) -> &'static str {
        match self {
            Refusal::Busy => {
                ": not started, another operation on this host is in progress; try again"
            }
            Refusal::LaunchInFlight => {
                ": not started, a session launch is in progress; try again when it finishes"
            }
        }
    }
}

/// What the Hosts panel's line shows for removals: a Remove that was not started, and a refused permanent answer's
/// words. Each carries the entity id of the operation store current when it was recorded and shows only while that is
/// the current store: a reset drops everything that came from the helm (SPEC_impl.md, "One failure path: the reset"),
/// and a new store has a new id. The panel redraws when it changes ([`observe_panel_lines`]).
#[derive(Default)]
struct PanelLines {
    not_started: Option<(EntityId, HostSentence)>,
    stop_asking: Option<(EntityId, HostSentence)>,
}

impl Global for PanelLines {}

/// Change the panel's lines with `change`.
fn show(cx: &mut App, change: impl FnOnce(&mut PanelLines)) {
    change(cx.default_global::<PanelLines>());
}

/// The entity id of the current UI session's operation store, which a line is recorded against.
fn current_store_id(cx: &App) -> Option<EntityId> {
    session::current_operations(cx).map(|operations| operations.entity_id())
}

/// Record that `removal` was not started, for `why`.
fn not_started(removal: &Removal, why: Refusal, cx: &mut App) {
    let Some(store) = current_store_id(cx) else {
        return;
    };
    let line = HostSentence::new(SharedString::from("hosts-remove-not-started"))
        .text("Remove ")
        .host(&removal.name)
        .text(why.words());
    show(cx, |lines| lines.not_started = Some((store, line)));
}

/// Redraw the Hosts panel whenever its removal lines change, for as long as it keeps the subscription: the global
/// notifies no view by itself.
pub(super) fn observe_panel_lines(cx: &mut Context<super::HostsPanel>) -> Subscription {
    cx.observe_global::<PanelLines>(|_, cx| cx.notify())
}

/// The Hosts panel's removal lines, as sentences under their own ids: a Remove that was not started, and a refused
/// permanent answer while the preference it tried to turn on is still off. Each only while it came from the current UI
/// session's store. Empty when there is neither.
pub(super) fn panel_lines(cx: &App) -> Vec<HostSentence> {
    let Some(lines) = cx.try_global::<PanelLines>() else {
        return Vec::new();
    };
    let current = current_store_id(cx);
    let not_started = lines
        .not_started
        .as_ref()
        .filter(|(store, _)| Some(*store) == current)
        .map(|(_, line)| line.clone());
    let stop_asking = lines
        .stop_asking
        .as_ref()
        .filter(|(store, _)| Some(*store) == current && asks_first(cx))
        .map(|(_, line)| line.clone());
    not_started.into_iter().chain(stop_asking).collect()
}
