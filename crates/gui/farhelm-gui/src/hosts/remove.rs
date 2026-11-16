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
//!   way to remove; the old dialog disabled both. Cancel returns focus to what held it when the question opened: the
//!   row's menu toggle, which the menu gave focus back to as it closed.
//! - **The request** ([`submit`]) is `DELETE /api/hosts/{id}`, the operation store's work under the host's key. Remove
//!   and the permanent answer keep the question up while it runs ([`prompt::ask_fallible`]), every answer drawn busy,
//!   as the old dialog stayed up through its request. A success needs nothing here: the pushed host list drops the row
//!   before the helm's reply arrives, the question closes with its subject, and focus recovers as from any control that
//!   goes (SPEC.md, "Where focus rests"). A refusal stays inside the question: "Farhelm refused this action:" and the
//!   helm's words under the body, in the prompt's amber, with focus moved to Cancel, as the old dialog's refusal did,
//!   and Remove pressed again retries. The question shows it, so it is dropped from the store's record under the host's
//!   key, and the row neither shows it again nor opens for it. A removal without asking has no question, so its refusal
//!   is the store's record, which the Hosts panel shows under the host's row, opening it.
//! - **Not started.** A Remove refused before anything is sent says why, naming the host: the host's key already has
//!   work in flight (the store's refusal, "Not started:" and the store's shared reason), or a session launch is in
//!   flight ([`super::launch_in_flight`], shared with Add host, setup and Re-run). An answer on the question says it
//!   inside the question, which stays up as for a refusal; the menu's Remove and a removal without asking say it on the
//!   Hosts panel's line until the next Remove or a reset ([`panel_lines`]). The launch rule is the migration's, decided
//!   by its orchestrator on 2026-11-14 after the old app refused a host change during a launch and the reverse: each
//!   side refuses its own action while the other's work is in flight in the operation store, with a reason, and no
//!   shared lock.
//! - **The permanent answer's preference.** "Remove, don't ask again" also turns the preference on, under the
//!   preference row's key, but only once its removal has been sent: a removal not started changes no preference. A
//!   removal the helm then refuses leaves the preference on: the user chose not to be asked again, the old app stored
//!   it before sending anything, and the question that stays up still offers both answers. The next Remove from the
//!   host menu then goes without asking, until Settings' switch turns the preference off. A second press of the
//!   permanent answer after such a refusal saves nothing once the helm lists the preference on. The removal goes ahead
//!   whether or not that save succeeds, so a refusal of the save shows on the panel's line, "Remove hosts without
//!   asking:" and the helm's words, or Settings' words for a press refused that way ([`Refused::WORDS`], "Not saved:"
//!   and the store's shared reason) when the operation store refused it because another preference save was in flight,
//!   until the next permanent answer, a reset, or the preference turning on.
//!
//! NOTE: Three of the recorded deviations (`docs/gpui/parity/settings-and-hosts.md`). The old dialog turned its summary
//! into a red "Farhelm refused this action" and drew the reason below the explanation in the body's ink; the prompt's
//! summary is frozen, so those words lead the reason, which the prompt draws in its amber at the frame's text size.
//! With the preference on, the old app opened a short refusal dialog, which is not ported yet; that refusal shows under
//! the row. And the old header drew the phase with its colored dot; the prompt's summary is a line of text, so the
//! phase words and build are drawn in the summary's ink.
//!
//! A Tensorlake or container host's removal deletes its machine. The menu offers it as the provider's own command,
//! Delete sandbox or Delete container ([`super::offers::HostKind::provider`]), which takes the same path with the
//! provider's words (the old `RemoveCopy::for_host`): the question "Delete sandbox · <name>", the machine's name in the
//! body where an SSH host's destination is, then "<name> Delete the whole sandbox? Every session, … This cannot be
//! undone." (`crate::environment_ui::ProviderUi::delete_consequence`), and the answer "Delete sandbox". It always asks,
//! whatever "Remove hosts without asking" says, and offers no permanent answer: that preference covers forgetting an
//! entry, and deleting a machine takes its files and sessions with it. The same `DELETE /api/hosts/{id}` goes out, and
//! the helm answers within its reply bound, so a deletion still running then is refused with "the removal carries on",
//! shown in the question as any refusal is, while the pushed hosts list shows how it ends.

#[cfg(test)]
mod tests;

use std::cell::Cell;
use std::rc::Rc;

use farhelm_helm_client::{ApiPath, Method, PathError};
use farhelm_proto::ProviderKind;
use farhelm_proto::ui_channel::{HostView, PREFERENCES_KEY, Preferences};
use gpui::{
    App, AppContext as _, Context, EntityId, Global, IntoElement, ParentElement as _, Render,
    SharedString, Styled as _, Subscription, Window, div, px,
};

use super::identity::{display_name, host_mark, phase};
use super::listed;
use super::offers;
use super::setup::UNDO_NOTE;
use crate::assets;
use crate::components::{HoverHelp, Tier};
use crate::environment_ui::ProviderUi;
use crate::focus::Mark;
use crate::host_text::{self, HostSentence, HostText};
use crate::layers::{LayerId, Subject};
use crate::operations::{Command, NotStarted, OpKey, Outcome, Work};
use crate::prompt::{self, Answer, Attempt, Question};
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
/// which a line about a removal that was not started names it by, its destination, which the body shows (for a machine
/// Farhelm deletes, the machine's own name, which is what the user can look up), and the provider whose machine the
/// removal deletes, if it deletes one.
#[derive(Clone)]
struct Removal {
    host: i64,
    name: HostText,
    destination: HostText,
    deletes: Option<ProviderKind>,
}

/// The host menu's Remove (or Delete sandbox, Delete container) for `host`, pressed with `mark` taken in its handler:
/// ask the confirmation, or with "Remove hosts without asking" on and a removal that only forgets the host, send it at
/// once. Refused with the panel line's reason while a session launch is in flight; nothing happens when the window is
/// inert or the registry no longer lists the host.
pub(super) fn start(host: i64, mark: &Mark, window: &mut Window, cx: &mut App) {
    let Some(view) = listed(host, cx) else {
        return;
    };
    let removal = Removal::of(&view);
    // A line from an earlier Remove belongs to that attempt; this one says its own.
    show(cx, |lines| lines.not_started = None);
    if super::launch_in_flight(cx) {
        not_started(&removal, Refusal::LaunchInFlight, None, cx);
    } else if removal.deletes.is_some() || asks_first(cx) {
        let _ = ask(host, mark, window, cx);
    } else {
        // TODO: with the preference on, the old app showed a refusal of this removal in the short host refusal dialog
        // ("Remove host · <name>", Close), opened through `mark`; it shows under the row until this opens that dialog
        // (`super::refusal`, which Update's refusals brought).
        submit(&removal, None, cx);
    }
}

impl Removal {
    /// The removal of `view` as the registry lists it now.
    fn of(view: &HostView) -> Removal {
        // The machine's own name for a machine Farhelm deletes (a recipe can name several), else the destination in
        // full, since the title may shorten the name (SPEC.md: "A shortened heading retains the full destination in
        // the body").
        let machine = view
            .environment
            .as_ref()
            .map(|environment| environment.name.as_str())
            .or_else(|| {
                view.tensorlake
                    .as_ref()
                    .map(|sandbox| sandbox.sandbox.as_str())
            });
        let destination = machine
            .or(view.destination.as_deref())
            .unwrap_or(&view.name);
        Removal {
            host: view.id,
            name: display_name(view),
            destination: HostText::from_host(destination),
            deletes: offers::HostKind::of(view).provider(),
        }
    }

    /// The command's label: "Remove", or the provider's Delete ("Delete sandbox").
    fn label(&self) -> &'static str {
        match self.deletes {
            Some(provider) => ProviderUi(provider).host_delete_label(),
            None => "Remove",
        }
    }
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
        let offered = offers::offers(&view, collections.get(&key), in_flight);
        // Each removal is enabled by its own offer: the menu offers Remove or the provider's Delete, never both.
        let enabled = match offers::HostKind::of(&view).provider() {
            Some(_) => offered.delete.is_some_and(|(_, enabled)| enabled),
            None => offered.remove == Some(true),
        };
        (view, enabled)
    };
    let removal = Removal::of(&view);
    let name = removal.name.clone();
    let (title, help) = match removal.deletes {
        Some(provider) => {
            let words = ProviderUi(provider);
            (words.host_delete_label(), words.host_delete_help())
        }
        None => ("Remove host", REMOVE_HELP),
    };
    let title = HostSentence::new("host-remove-title")
        .text(format!("{title} · "))
        .host(&name);
    let summary = phase_summary("host-remove-summary", &view);
    let (icon, color) = (view.icon, view.color);
    let remove = Answer::new(removal.label(), HoverHelp::tooltip(help), Tier::Danger);
    let deletes = removal.deletes.is_some();
    let mut question = Question::new(title, remove, removal)
        .summary(summary)
        .body(|removal: &Removal, _, cx| {
            let explanation = match removal.deletes {
                // The machine's name, then what deleting it takes with it.
                Some(provider) => HostSentence::new("host-remove-explanation")
                    .host(&removal.destination)
                    .text(format!(" {}", ProviderUi(provider).delete_consequence())),
                None => HostSentence::new("host-remove-explanation").text(EXPLANATION),
            };
            let body = RemoveBody {
                destination: removal.destination.clone(),
                explanation,
            };
            cx.new(|_| body)
        })
        .confirm_enabled(enabled)
        .cancel_answer(Answer::new(
            "Cancel",
            HoverHelp::tooltip("Cancel: keep this host"),
            Tier::Neutral,
        ));
    // Only beside an available Remove: with Remove unavailable it would be the one way left to remove the host. The
    // footer note names this answer, so it goes with it. Never for a machine's deletion, which always asks.
    if enabled && !deletes {
        question = question.note(UNDO_NOTE).fallible_alternative(
            Answer::new(
                "Remove, don't ask again",
                HoverHelp::tooltip("Remove it, and remove hosts without asking from now on"),
                Tier::DangerOutline,
            ),
            |removal, attempt, _, cx| {
                // The preference follows the removal it answers once that is sent: a removal refused before it was
                // sent changes nothing, and one the helm then refuses leaves the preference on (the module docs). A
                // retry after that refusal saves nothing once the helm lists the preference on, since a second save
                // would only repeat the first. Before then it still sends one, which the store may refuse as busy
                // while the first is out.
                if submit(&removal, Some(attempt), cx) && asks_first(cx) {
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
    // The subject going is also how a successful removal closes the question: the helm's reply follows the pushed host
    // list that no longer names the host.
    let subject = Subject::when(store, move |store, _| super::settings::gone(store, &key));
    prompt::ask_fallible(
        question,
        Some(subject),
        mark,
        |removal, attempt, _, cx| {
            submit(&removal, Some(attempt), cx);
        },
        window,
        cx,
    )
}

/// The summary line under a host confirmation's title, the old `HostDialogHeader`'s for a host: `view`'s phase words,
/// then the build it reports ("Connected · Farhelm 0.16.0"), the build in the monospace face as host text, under the
/// sentence id `id`. The removal and uninstall confirmations both show it, frozen as they opened.
pub(super) fn phase_summary(id: &'static str, view: &HostView) -> HostSentence {
    let summary = HostSentence::new(id).text(phase::words(&view.state));
    match phase::reported_build(&view.state) {
        Some(build) => summary
            .text(" · Farhelm ")
            .mono(&HostText::from_host(build)),
        None => summary,
    }
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
/// is busy (the module docs). Returns whether the removal was sent, which the permanent answer's preference waits on.
///
/// `attempt` is the confirmation's, which stays up until it settles; `None` is a removal without asking. With an
/// attempt, every outcome settles it: a removal not started, or one the helm refused, fails it with the reason, which
/// the question then shows, and the helm's answer otherwise succeeds it. Without one, a removal not started says why on
/// the panel's line, and the store's record of a refusal under the host's key is the report, where the Hosts panel
/// shows it.
fn submit(removal: &Removal, attempt: Option<Attempt>, cx: &mut App) -> bool {
    if super::launch_in_flight(cx) {
        not_started(removal, Refusal::LaunchInFlight, attempt, cx);
        return false;
    }
    // An inert window has no store, and the reset that made it inert closed every layer, the question included, so an
    // attempt dropped here has nothing left to hold open.
    let Some(operations) = session::current_operations(cx) else {
        return false;
    };
    let command = match remove_command(removal.host) {
        Ok(command) => command,
        Err(error) => {
            // A registry id always makes a path segment, so this is a bug report. Closing the question keeps it from
            // staying frozen on an attempt that nothing would settle.
            tracing::debug!(?error, "host removal not built");
            if let Some(attempt) = attempt {
                attempt.succeeded(cx);
            }
            return false;
        }
    };
    let key = OpKey::host(&HostView::key(removal.host));
    // Shared with the completion, which the store keeps only when the removal goes out: a removal it refuses to start
    // takes the attempt back to say why.
    let attempt = Rc::new(Cell::new(attempt));
    let settling = Rc::clone(&attempt);
    let refused_key = key.clone();
    let started = operations.update(cx, |operations, cx| {
        operations.start(&key, command, cx, move |settled, cx| {
            let Some(attempt) = settling.take() else {
                return;
            };
            match &settled.outcome {
                Outcome::Refused(refusal) => {
                    attempt.failed(refused_line(&refusal.words), cx);
                    // Shown inside the question, which is still up: it closes mid-attempt only when its host is gone,
                    // and then no row draws the record either, or on a reset, which runs no completion. Dropped from
                    // the store's record so the row does not report the one refusal a second time.
                    if let Some(operations) = session::current_operations(cx) {
                        operations.update(cx, |operations, cx| operations.forget(&refused_key, cx));
                    }
                }
                Outcome::Done(_) | Outcome::Accepted(_) => attempt.succeeded(cx),
            }
        })
    });
    match started {
        Ok(()) => {
            show(cx, |lines| lines.not_started = None);
            true
        }
        Err(NotStarted::Busy(_)) => {
            not_started(removal, Refusal::Busy, attempt.take(), cx);
            false
        }
        // A reset retired the store: the generic connection-lost notice is the report, and the reset closes the
        // question.
        Err(not_started) => {
            tracing::debug!(?not_started, "host removal not sent");
            false
        }
    }
}

/// What the confirmation shows under its body when the helm refuses the removal: the old dialog's "Farhelm refused
/// this action", led into the helm's own words, which may quote host values.
fn refused_line(words: &HostText) -> HostSentence {
    HostSentence::new("host-remove-refused")
        .text(REFUSED_LEAD_IN)
        .host(words)
}

/// The old dialog's refusal summary, which here leads the reason since the question's summary is frozen as it opened.
const REFUSED_LEAD_IN: &str = "Farhelm refused this action: ";

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
                let store = super::current_operations_id(cx);
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
            let store = super::current_operations_id(cx);
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
    /// What removing does: forgetting the host, or for a machine Farhelm deletes, its name and what goes with it.
    explanation: HostSentence,
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
                    .child(self.explanation.clone()),
            )
    }
}

// ---- Removals not started, and the Hosts panel's line ---------------------------------------------------------------

/// Why a Remove was not started.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Refusal {
    /// The host's key already has work in flight: the operation store refused the request.
    Busy,
    /// A session launch is in flight (the module docs).
    LaunchInFlight,
}

impl Refusal {
    /// What the line says after the host's name and its colon. A busy key gets "Not started: " and the operation
    /// store's shared reason ([`NotStarted::BUSY_REASON`]), which every area gives after a lead-in of its own, so a
    /// Remove the store refused reads like any other press it refused; the migration's orchestrator decided that split
    /// on 2026-11-14. A launch in flight keeps the old app's form for an action refused while another runs ("<action>:
    /// not started, another operation is in progress; try again", `list/lifecycle.rs` at the reference commit), naming
    /// the launch, as every host registry change does.
    fn words(self) -> String {
        match self {
            Refusal::Busy => format!("{BUSY_LEAD_IN}{}", NotStarted::BUSY_REASON),
            Refusal::LaunchInFlight => super::LAUNCH_IN_FLIGHT.to_owned(),
        }
    }
}

/// What a Remove refused for a busy host key did not do, ahead of the store's reason ([`Refusal::words`]).
const BUSY_LEAD_IN: &str = "Not started: ";

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

/// Say that `removal` was not started, for `why`: inside the confirmation by failing its `attempt`, or, for a Remove
/// that asked nothing, on the panel's line. The words are the same in both places, under an id of each place's own.
fn not_started(removal: &Removal, why: Refusal, attempt: Option<Attempt>, cx: &mut App) {
    let line = |id: &'static str| {
        HostSentence::new(SharedString::from(id))
            .text(format!("{} ", removal.label()))
            .host(&removal.name)
            .text(format!(": {}", why.words()))
    };
    if let Some(attempt) = attempt {
        attempt.failed(line("host-remove-not-started"), cx);
        return;
    }
    let Some(store) = super::current_operations_id(cx) else {
        return;
    };
    let line = line("hosts-remove-not-started");
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
    let current = super::current_operations_id(cx);
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
