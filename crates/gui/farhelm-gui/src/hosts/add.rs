//! The Add host dialog: an SSH destination and an optional alias, checked by the helm's probe before anything is
//! added, or a Tensorlake sandbox created from a recipe ([`tensorlake`]). The
//! Tailscale checkbox is part of the SSH draft and follows it through sign-in.
//!
//! SPEC.md, "Topology" (the add host dialog, the host registry and its duplicate refusal). Ported from the old app's
//! `hosts.rs` (`AddHostForm`, `validate_alias_draft`) and `api.rs` (`probe_ssh_host`), with its look from the
//! `.host-add-dialog`, `.add-host-form` and `.host-dialog-help` rules in `app.css`, all at reference commit `24bc0580`.
//! Here's the TLDR of its rules:
//!
//! - **Opening.** [`open`] is the entry point for the host rail's Add host and the Hosts panel heading's add button.
//!   The opener gets focus back when the dialog closes, as Settings' gear does, because the dialog names it as its
//!   return target. Focus starts in the destination field, or in a reopened sandbox's Name: the sandbox's creation
//!   question reopens Add host on the Tensorlake sandbox kind for its Back and for a refused creation
//!   ([`reopen_sandbox`]), with the name, the recipe and the account the form had.
//! - **The host kind.** Above the fields, a strip of two choices, SSH and Tensorlake sandbox, picks what the dialog
//!   adds. SSH, the fields below, is where it opens. Tensorlake sandbox swaps in the sandbox's own body
//!   ([`tensorlake`]), its Continue in Add's place, its main action for Enter on the dialog itself, and the header's
//!   summary in its words; choosing SSH again brings back the SSH fields as they were and drops the sandbox's form with
//!   its draft, as the old dialog's remount did, so choosing the sandbox again starts it afresh and reads the account
//!   again. The choice is not drawn for an add that began as an agent's request, which asks for an SSH destination,
//!   nor while the sandbox's recipe editor is open. While Add's probe or answer is out a choice does nothing, as the
//!   old dialog's other kind was unavailable then; the strip has no unavailable state, so its arrows still move focus
//!   to the other tab there, which chooses nothing (recorded in `docs/gpui/parity/settings-and-hosts.md`,
//!   "Deviations").
//! - **Before the probe is sent.** Add refuses an alias the helm would refuse for its shape (the old app's
//!   `validate_alias_draft`, in its words) and a destination that is already a host entry, naming that entry in the
//!   helm's own words (SPEC.md: "Adding a destination that is already a host entry is refused, naming that entry,
//!   before anything contacts the destination"). The helm refuses both before it contacts anything too, so the app's
//!   checks are not what keeps the destination from being dialed. They are there to answer at once: the helm runs a
//!   probe's checks only once the probe holds one of its few planning slots, which other hosts' probes may be holding,
//!   so its refusal can come only after them, or as the timeout reply. The duplicate check reads the pushed hosts and
//!   compares the destination exactly as typed, as the helm does; the helm checks both again, so a host added elsewhere
//!   a moment ago is still refused, by the helm's reply instead.
//! - **During a launch.** Add then refuses to probe while a session launch is in flight anywhere in the operation store
//!   (the launcher's New or Clone, a session's Restart or Replace: [`super::launch_in_flight`]), and says so in the
//!   error line ("Add host: " and [`super::LAUNCH_IN_FLIGHT`]) with both fields kept. The old app refused a host change
//!   during a launch through one page-wide token; here each side refuses its own action while the other's work is in
//!   flight (decided by the migration's orchestrator on 2026-11-14), and the launcher's refusal of a launch during host
//!   work is its own. Checked after the two checks above, so the line names a mistake in the fields, which waiting will
//!   not fix, first.
//! - **The probe.** Add, Enter in either field, or Enter on the dialog itself sends `POST /api/hosts/probe` for the
//!   destination with a `new_entry` carrying the alias ([`probe_command`]), as the operation store's work under one
//!   [`OpKey::new_host_add`] key per opening of the dialog, which is the key the setup submission continues under once
//!   it is built. One request per key, so a second Add while the probe is out sends nothing. Meanwhile both fields are
//!   read-only and Add is busy, reading "Checking…"; Cancel, the header's close control and Escape stay live, and
//!   closing drops only the dialog's wait (the probe goes on in the helm, and a supervisor that answers is still
//!   registered and shows up in the pushed hosts).
//! - **The outcomes.** A running supervisor is registered as it is (`discovered`) and the dialog closes. Why setup is
//!   manual (`manual`), the helm's refusal (a duplicate or taken alias, an unusable destination, the 504 that says the
//!   check carries on), and a reply that cannot be read all show in the dialog's error line, both fields kept for
//!   correction. A destination with no supervisor (`provisionable`) goes on to setup; [`super::setup`]'s module docs
//!   say how, and when a refused setup reopens the dialog ([`reopen`]).
//! - **SSH sign-in.** A pending setup question appears below the destination and keeps the draft while it waits.
//!   The draft holds the check until the matching setup master answers. Cancel ends the check without clearing the
//!   fields. If a supervisor was added, its sessions connection can ask after the dialog closes.
//!
//! - **An agent's request.** An agent's `farhelm host add` waits on an approval card, whose action opens this dialog
//!   ([`open_for_agent`]) prefilled with the destination and alias the agent asked for, and labelled with who asked;
//!   opening is not approval (SPEC.md, "Agent-spawned sessions"). Add answers the request's Allow with the destination
//!   then in the field, after the same checks as any Add, and the probe starts only once the helm accepted it (the
//!   approvals module's [`AgentRequest`] sends the answer, under the request's own key). Cancel, the header's close and
//!   Escape answer Deny through the dialog's dismiss hook, and the dialog closes once the helm took it; after an
//!   accepted Allow they are ordinary cancellation and answer nothing. An agent's add always asks the setup
//!   confirmation ([`Seed::from_agent`], carried through a refused setup's reopen too), whatever the setup preference
//!   says. The dialog closes when its request stops waiting before it was accepted, as the old dialog did, a late
//!   Allow included. While another dialog is open, or a host is being probed, set up or removed, the open is refused
//!   with a reason ([`NotOpened::reason`]); the old app queued it.
//!
//! An outcome shows in the dialog whether or not the user moved on since Add: drawing a line inside the dialog that is
//! already open moves no focus and opens nothing, which is all SPEC.md's "Keyboard focus" rule withholds from a late
//! result. `discovered` closes the dialog either way, since the dialog is its own subject (the rule
//! [`crate::operations::Settled::user_moved_on`] documents for Rename's success), and the layer manager returns focus
//! only if it is still inside. If the user closed the dialog before the reply, the reply reaches nobody. The setup
//! confirmation is the one outcome that would open something; [`super::setup`] says how the same rule applies to it.

mod tensorlake;
#[cfg(test)]
mod tensorlake_tests;
#[cfg(test)]
pub(crate) mod tests;

use std::rc::Rc;

use farhelm_helm_client::{ApiPath, Method, PathError};
use farhelm_proto::HelmOwnerRefusal;
use farhelm_proto::host_appearance::{HostColor, HostIcon};
use farhelm_proto::provisioning::{PlanSteps, ProbeReply};
use farhelm_proto::ui_channel::{HostView, SshQuestion, SshQuestionTarget};
use gpui::{
    AnyElement, AnyWindowHandle, App, AppContext as _, ClickEvent, Context, Entity, FocusHandle,
    IntoElement, ParentElement as _, Render, Styled as _, Subscription, WeakEntity, Window, div,
    px,
};
use serde::Deserialize as _;
use serde::de::IgnoredAny;
use serde_json::{Value, json};

use super::{setup, setup_sign_in};
use crate::approvals::answers;
use crate::approvals::host_add::{self, AgentRequest, Answered};
use crate::components::{
    Button, Checkbox, DialogFrame, HoverHelp, Role, TabOption, TabStrip, TextInput, TextInputState,
    Tier,
};
use crate::drafts::DraftInput;
use crate::focus::{self, Mark, ReturnTo};
use crate::host_text::{HostSentence, HostText};
use crate::hosts::identity::{display_name, host_mark};
use crate::layers::{self, Dialog, Dismissal, LayerId, MainAction, Subject};
use crate::operations::{Command, OpKey, Outcome, Settled, Work};
use crate::prompt::{self, Answer, Question};
use crate::session;
use crate::sign_in_form::{PrepareHandoff, SignInForm};
use crate::theme::{self, metrics};
use tensorlake::TensorlakeForm;
pub(super) use tensorlake::{ACCOUNT_ROUTE, read_plan};

/// `.host-add-dialog`'s width.
const WIDTH: f32 = 560.;

/// `.host-settings-body`'s gap between the fields, their help and the error line.
const BODY_GAP: f32 = 20.;

/// `.add-host-form label`'s gap between a field's label and its input.
const LABEL_GAP: f32 = 6.;

/// `.add-host-form input`'s `min-height`: the fields' height.
const FIELD_HEIGHT: f32 = 36.;

/// The header tile's mark: `HostMark`'s default 24 pixels.
const TILE_MARK: f32 = 24.;

/// What Cancel and the header's close control say on hover, the old dialog's words.
const CANCEL_HELP: &str = "Cancel: close without adding a host";

/// The helm's limit on an alias's length, in characters (its `validate_alias`, which the old app mirrored).
const ALIAS_MAX_CHARS: usize = 64;

/// The hover help of every control that opens this dialog (the host rail's Add host and the Hosts panel heading's add
/// button), the old buttons' words. One copy, so the entry points cannot drift apart.
pub(crate) const ADD_HOST_HELP: &str = "Add host: connect a machine or create a Tensorlake sandbox";

/// What the dialog adds, as its kind choice says (the module docs): the kind choice's values.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    /// A machine reached over SSH: the destination and alias fields, and Add.
    Ssh,
    /// A Tensorlake sandbox created from a recipe ([`tensorlake`]).
    Tensorlake,
}

/// What the dialog shows below its kind choice.
enum Body {
    /// The SSH destination and alias. Their drafts live on the dialog itself, so they outlast a switch of kind.
    Ssh,
    /// A Tensorlake sandbox's form: made each time the kind is chosen and dropped when SSH is chosen again, with its
    /// draft, as the old dialog remounted its Tensorlake form on each switch.
    Tensorlake(Entity<TensorlakeForm>),
}

impl Body {
    /// The kind this body is for, which the kind choice shows as chosen.
    fn kind(&self) -> Kind {
        match self {
            Body::Ssh => Kind::Ssh,
            Body::Tensorlake(_) => Kind::Tensorlake,
        }
    }
}

/// Open the Add host dialog on top of everything else in `window`: the entry point for the host rail's Add host and
/// the Hosts panel heading's add button.
///
/// `opener` is the clicked control's focus handle and `mark` the [`Mark`] taken in its click handler. The dialog names
/// the opener as where closing returns focus ([`ReturnTo::Control`]), so it goes back there on every platform, also
/// where a click leaves a button unfocused (macOS), as [`crate::settings::open`] does for the gear (SPEC.md: "Closing
/// something the user opened returns focus to where it was before it opened, on every platform"). Nothing opens when
/// the user has moved on since `mark` or the window is inert.
///
/// Returns the dialog's layer, or `None` when nothing opened.
pub fn open(
    opener: &FocusHandle,
    mark: &Mark,
    window: &mut Window,
    cx: &mut App,
) -> Option<LayerId> {
    let layer = reopen(opener, mark, None, window, cx)?;
    // A new add: what the Hosts panel's line says about an earlier one goes, as the old panel's error did.
    setup::forget_panel_lines(cx);
    Some(layer)
}

/// What a reopened dialog starts from: the fields as they were sent and why the setup the user confirmed was refused
/// ([`reopen`]).
pub(crate) struct Seed {
    pub(crate) destination: String,
    pub(crate) alias: String,
    pub(crate) tailscale: bool,
    pub(crate) problem: HostSentence,
    /// The add began as an agent's request, which the user accepted: the reopened dialog still shows what Add
    /// approves, and still asks the setup confirmation whatever the setup preference says (SPEC.md: the confirmation
    /// always shows for an agent's add, "including after a refused setup submission is reopened").
    pub(crate) from_agent: bool,
}

/// What a reopened dialog on the Tensorlake sandbox kind starts from ([`reopen_sandbox`]): the name and recipe its
/// creation confirmation was for, the account as the form had read it (so Name and Recipe show at once, with focus in
/// Name), and why it reopened, if it is a refusal rather than the confirmation's Back.
pub(crate) struct SandboxSeed {
    pub(crate) name: String,
    pub(crate) recipe: String,
    pub(crate) account: farhelm_proto::tensorlake::Settings,
    pub(crate) problem: Option<HostSentence>,
}

/// Why [`open_for_agent`] opened nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NotOpened {
    /// Another dialog is open; the card says so ([`NotOpened::reason`]) and the request keeps waiting.
    AnotherDialog,
    /// A host is being probed, set up or removed; the card says so ([`NotOpened::reason`]) and the request keeps
    /// waiting.
    HostChangeInFlight,
    /// Answers are paused, an answer to the request is in flight, the request no longer waits, the user moved on since
    /// the press, or the window is inert: nothing to say.
    Nothing,
}

impl NotOpened {
    /// What the card's refusal line says, in the app's words, or `None` when there is nothing to say. The old app queued
    /// the open behind the open dialog or the host change instead; this port refuses it and keeps both the open dialog
    /// and the request (a recorded deviation), so the user can finish what is open and act on the card again.
    pub fn reason(self) -> Option<&'static str> {
        match self {
            NotOpened::AnotherDialog => {
                Some("Another dialog is open. Close it, then open this request's Add host again.")
            }
            NotOpened::HostChangeInFlight => Some(
                "Another host change is still in progress. Open this request's Add host again once it has finished.",
            ),
            NotOpened::Nothing => None,
        }
    }
}

/// Open the Add host dialog for an agent's host add request, from its card's action: prefilled with the destination
/// and alias the agent asked for and labelled with who asked, Add answering the request's Allow and Cancel its Deny
/// (the module docs). Focus returns to `opener`, the card's control, as [`open`] returns it to its button. The dialog
/// belongs to the card, not to any layer ([`open_with`] says why).
///
/// Refused while another dialog is open, keeping that dialog and its draft (SPEC.md: "an existing draft or another
/// modal is kept"), and while a host is being probed, set up or removed, so that an ordinary add's refused setup can
/// still reopen its own draft. The old app queued such an open until both were over instead; this port refuses it
/// with a reason, recorded as a deviation, until a dialog that waits for the slot exists.
pub fn open_for_agent(
    request: AgentRequest,
    opener: &FocusHandle,
    mark: &Mark,
    window: &mut Window,
    cx: &mut App,
) -> Result<LayerId, NotOpened> {
    // A layer that holds focus is what the user is in; a popover that never takes focus (a link's destination
    // display) is not a dialog to keep.
    if layers::focus_layer_open(window, cx) {
        return Err(NotOpened::AnotherDialog);
    }
    if super::host_change_in_flight(cx) {
        return Err(NotOpened::HostChangeInFlight);
    }
    if request.paused(cx) || request.answering(cx) || !request.waiting(cx) {
        return Err(NotOpened::Nothing);
    }
    // Unlike the rail's Add host, the Hosts panel's line about an earlier add stays: the old app cleared it only for an
    // add the user started there.
    open_with(opener, mark, Start::Agent(request), window, cx).ok_or(NotOpened::Nothing)
}

/// What a dialog opens with.
enum Start {
    /// Empty, from the host rail or the Hosts panel.
    Empty,
    /// As a refused setup left it ([`reopen`]).
    Seed(Seed),
    /// For an agent's request ([`open_for_agent`]).
    Agent(AgentRequest),
    /// On the Tensorlake sandbox kind, as its creation confirmation left it ([`reopen_sandbox`]).
    Sandbox(SandboxSeed),
}

/// [`open`], with the fields and the error line taken from `seed` when there is one: how a setup the helm refused
/// opens the dialog again, as the old app did ("a refused ADD attempt reopens the form"), with the destination and
/// alias as they were sent and the helm's reason in the error line, but not the plan, which the helm consumed; Add
/// probes again. The setup's completion calls it only while the user has not moved on since the action that sent the
/// plan, through that action's mark ([`setup::Offer::mark`]). Focus returns to `opener`, the control that opened the
/// first dialog. Unlike [`open`], it leaves the Hosts panel's line alone: a reopen continues the same add.
pub(crate) fn reopen(
    opener: &FocusHandle,
    mark: &Mark,
    seed: Option<Seed>,
    window: &mut Window,
    cx: &mut App,
) -> Option<LayerId> {
    let start = match seed {
        Some(seed) => Start::Seed(seed),
        None => Start::Empty,
    };
    open_with(opener, mark, start, window, cx)
}

/// [`reopen`] on the Tensorlake sandbox kind: how the sandbox's creation confirmation goes back to Add host, for its
/// Back or a refusal of the creation (`setup`'s module docs), with the name and recipe as they were, `seed`'s problem
/// in the error line, and focus in Name. Focus returns to `opener`, the control that opened the first dialog. Nothing
/// opens once the user has moved on since `mark`.
pub(crate) fn reopen_sandbox(
    opener: &FocusHandle,
    mark: &Mark,
    seed: SandboxSeed,
    window: &mut Window,
    cx: &mut App,
) -> Option<LayerId> {
    open_with(opener, mark, Start::Sandbox(seed), window, cx)
}

/// Open the dialog as `start` says, unless the user moved on since `mark` or the window is inert. An agent's dialog
/// also gets the dismiss hook that answers Deny and the subject that closes it once its request stops waiting.
///
/// An agent's dialog, first opened from its card or reopened after a refused setup, belongs to the request's card and
/// to no layer ([`layers::opened_from_approval_card`], whose contract names both): another layer's close never takes
/// it. The mark keeps that contract, and takes effect once Add host can open over another dialog. Until then
/// [`open_for_agent`] refuses that, and no layer holding focus can become the dialog's parent anyway.
fn open_with(
    opener: &FocusHandle,
    mark: &Mark,
    start: Start,
    window: &mut Window,
    cx: &mut App,
) -> Option<LayerId> {
    // An inert window has no hosts to check against and nothing to probe through.
    let store = session::current_store(cx)?;
    if focus::has_moved_on(mark, cx) {
        return None;
    }
    let agent = matches!(start, Start::Agent(_));
    let dialog = cx.new(|cx| AddHostDialog::new(opener.clone(), start, window, cx));
    let start = dialog.read(cx).start(cx);
    let weak = dialog.downgrade();
    // Enter on the dialog itself is the shown kind's one main action: Add, or the sandbox's Continue.
    let main_action: MainAction = Rc::new(move |window, cx| {
        let _ = weak.update(cx, |dialog, cx| dialog.main_action(window, cx));
    });
    let layer = layers::open_dialog(
        Dialog {
            view: dialog.clone().into(),
            container: cx.focus_handle(),
            start,
            // An agent's dialog is about its request until the helm accepted it: gone once the request stops waiting
            // with no answer of the dialog's in flight (an answer settles before the dialog says what came of it).
            subject: agent.then(|| {
                let dialog = dialog.downgrade();
                Subject::when(store, move |_, cx| {
                    dialog
                        .upgrade()
                        .is_some_and(|dialog| dialog.read(cx).request_gone(cx))
                })
            }),
            main_action: Some(main_action),
            // SPEC.md, "Opening and closing": the dialogs that share the host and Settings installer keep focus on the
            // control in use when their empty space is clicked.
            keeps_focus_on_empty_click: true,
            returns_to: ReturnTo::Control(opener.clone()),
            // An ordinary add: closing never waits, not even for a probe in flight (the module docs), unless the
            // Tensorlake sandbox's recipe editor holds unsaved changes, which asks first. An agent's add answers Deny
            // first, and closes once the helm took it.
            on_dismiss: Some({
                let dialog = dialog.downgrade();
                let hook: layers::DismissHook = Rc::new(move |window, cx| {
                    dialog
                        .update(cx, |dialog, cx| dialog.dismissal(window, cx))
                        .unwrap_or(Dismissal::Close)
                });
                hook
            }),
            placement: layers::DialogPlacement::Centered,
            dismiss_on_outside_press: false,
        },
        mark,
        window,
        cx,
    )?;
    dialog.update(cx, |dialog, _| dialog.layer = Some(layer));
    if dialog.read(cx).from_agent {
        layers::opened_from_approval_card(layer, window, cx);
    }
    #[cfg(test)]
    layers::record_opened(&dialog, cx);
    Some(layer)
}

/// Open the recipe editor in the open Add host dialog's Tensorlake sandbox form, as Recipe's "New recipe…" does, for
/// the golden that pictures the editor inside Add host, as `settings::open_new_recipe_for_opened` does for Settings. A
/// golden's job is the picture: pressing Recipe and then "New recipe…" from the scene made it depend on where an
/// earlier frame drew each control, while the form's own tests already pin that click path. Panics when no dialog is
/// open or it shows the SSH kind, which would leave the golden picturing the form instead.
#[cfg(test)]
pub(crate) fn open_new_recipe_for_opened(window: &mut Window, cx: &mut App) {
    let form =
        layers::last_opened::<AddHostDialog>(cx).and_then(|dialog| match &dialog.read(cx).body {
            Body::Tensorlake(form) => Some(form.clone()),
            Body::Ssh => None,
        });
    match form {
        Some(form) => form.update(cx, |form, cx| form.open_editor(window, cx)),
        None => panic!(
            "no Add host dialog shows a Tensorlake sandbox form to open the recipe editor in"
        ),
    }
}

/// `POST /api/hosts/probe` for `destination` as a new host entry under `alias`: the destination exactly as typed, and
/// the alias as typed, or `null` when it is blank, which the helm reads as no alias (it trims an alias itself). No
/// install paths: the probe looks where setup installs, as it did for the old dialog, which stopped asking for them.
pub(crate) fn probe_command(
    destination: &str,
    alias: &str,
    tailscale: bool,
    from_agent: bool,
) -> Result<Command, PathError> {
    let alias = (!alias.trim().is_empty()).then_some(alias);
    let entry = if from_agent {
        json!({ "alias": alias, "from_agent": true })
    } else {
        json!({ "alias": alias })
    };
    let mut target = json!({ "kind": "ssh", "destination": destination });
    if tailscale {
        target["tailscale"] = json!(true);
    }
    Ok(Command::new(
        Work::Probe,
        Method::Post,
        ApiPath::api(&["hosts", "probe"])?,
        Some(json!({
            "target": target,
            "new_entry": entry,
        })),
        "POST /api/hosts/probe",
    ))
}

/// Why Add must not probe `destination` under `alias`, given the hosts the helm pushed, or `None` when it may: the
/// alias's shape first, as the old app checked it before sending, then a destination that is already an entry.
///
/// The duplicate's words are the helm's own for the same refusal, so the user reads the same sentence whichever side
/// caught it, with the entry's name as its row shows it drawn as host text inside the helm's quotes. The alias's words
/// are the old app's, which capitalized what the helm says in lower case. The module docs say why the app checks at
/// all when the helm checks the same before contacting anything.
fn refused_before_contact<'a>(
    destination: &str,
    alias: &str,
    hosts: impl IntoIterator<Item = &'a HostView>,
) -> Option<HostSentence> {
    let alias = alias.trim();
    if alias.chars().any(char::is_control) {
        return Some(error_line().text("Host alias must not contain control characters"));
    }
    if alias.chars().count() > ALIAS_MAX_CHARS {
        return Some(error_line().text("Host alias must be at most 64 characters"));
    }
    let twin = hosts
        .into_iter()
        .find(|host| host.destination.as_deref() == Some(destination))?;
    Some(
        error_line()
            .text("this host is already added as \"")
            .host(&display_name(twin))
            .text("\"; set it up again from that host's own row"),
    )
}

/// Why Add must not go on now, or `None` when it may: what [`refused_before_contact`] refuses, then a session launch
/// in flight ([`super::launch_in_flight`]), which waiting will fix, so it is named only once the fields are right.
fn refusal(destination: &str, alias: &str, cx: &App) -> Option<HostSentence> {
    session::current_store(cx)
        .and_then(|store| {
            refused_before_contact(
                destination,
                alias,
                store.read(cx).collections().ordered::<HostView>(),
            )
        })
        .or_else(|| {
            super::launch_in_flight(cx)
                .then(|| error_line().text(format!("Add host: {}", super::LAUNCH_IN_FLIGHT)))
        })
}

/// The dialog's error line, empty: what every outcome it shows starts from, a reopened dialog's included.
pub(crate) fn error_line() -> HostSentence {
    HostSentence::new("add-host-error")
}

/// What a probe's reply means, for the dialog and for a host row's rerun of its setup ([`super::setup::rerun`]).
pub(super) enum Reply {
    /// The helm registered an answering supervisor: the dialog closes.
    Added { host_id: i64 },
    /// Why the probe leads nowhere, to show: why setup is manual, a refusal, a reply that cannot be read.
    Show(Problem),
    /// The target answered with another owner. User Add needs its own
    /// confirmation before a registry entry exists.
    TakeOver {
        probe_id: String,
        owner: HelmOwnerRefusal,
    },
    /// No supervisor answered, and the helm offers this plan to set one up ([`setup`]).
    Setup {
        probe_id: String,
        confirmation: String,
        steps: PlanSteps,
    },
}

/// Why a probe's reply leads nowhere: the helm's own words (a refusal, why setup is manual), the app's for a reply it
/// cannot use, or the app's with the decoder's words for a reply it could not read.
pub(super) enum Problem {
    Helm(HostText),
    App(&'static str),
    Unreadable(HostText),
}

/// What [`Problem::Unreadable`] says before the decoder's words.
const UNREADABLE: &str = "The helm accepted the host probe, but its reply could not be read: ";

impl Problem {
    /// The problem appended to the empty `line`, each host value its own part.
    pub(super) fn sentence(self, line: HostSentence) -> HostSentence {
        match self {
            Problem::Helm(words) => line.host(&words),
            Problem::App(words) => line.text(words),
            Problem::Unreadable(error) => line.text(UNREADABLE).host(&error),
        }
    }

    /// The problem as one piece of host text, for the operation store's record under a host's key, which holds only
    /// host text (a refusal's words); the app's words in it are escaped too, which changes nothing.
    pub(super) fn text(self) -> HostText {
        match self {
            Problem::Helm(words) => words,
            Problem::App(words) => HostText::from_host(words),
            Problem::Unreadable(error) => {
                HostText::from_host(format!("{UNREADABLE}{}", error.raw()))
            }
        }
    }
}

/// Read a probe's reply. The probe never answers 202, but an accepted reply is read the same way as a done one, by its
/// body.
pub(super) fn read_reply(outcome: &Outcome) -> Reply {
    let body = match outcome {
        Outcome::Refused(refusal) => return Reply::Show(Problem::Helm(refusal.words.clone())),
        Outcome::Done(body) | Outcome::Accepted(body) => body.as_ref().unwrap_or(&Value::Null),
    };
    // The plan is the helm's own and never read here (`ProbeReply`'s docs): the setup confirmation shows the steps and
    // confirms by the probe's id, and the helm acts only on the plan it kept under that id.
    match ProbeReply::<IgnoredAny>::deserialize(body) {
        Ok(ProbeReply::Discovered { host_id, .. }) => Reply::Added { host_id },
        Ok(ProbeReply::OwnedElsewhere { probe_id, owner }) => Reply::TakeOver { probe_id, owner },
        Ok(ProbeReply::Manual { reason }) => {
            Reply::Show(Problem::Helm(HostText::from_host(reason.as_str())))
        }
        Ok(ProbeReply::Provisionable {
            probe_id,
            confirmation,
            steps,
            ..
        }) => Reply::Setup {
            probe_id,
            confirmation,
            steps: *steps,
        },
        // The old app's words for the same impossible reply to an SSH probe.
        Ok(ProbeReply::TensorlakeProvisionable { .. }) => Reply::Show(Problem::App(
            "The helm returned a Tensorlake plan for an SSH host",
        )),
        // The decoder's words quote the reply (an unknown variant's name, a string it did not expect), so they are host
        // text.
        Err(error) => Reply::Show(Problem::Unreadable(HostText::from_host(error.to_string()))),
    }
}

/// Setup reruns and agent Add may explain the refusal but cannot confirm it.
pub(super) fn ownership_problem(owner: &HelmOwnerRefusal) -> Problem {
    Problem::Helm(HostText::from_host(format!(
        "Another Farhelm ({}) manages this host.",
        owner.tag
    )))
}

/// A one-use takeover question carries the ownership report from the Add
/// probe. The normal setup preference cannot approve it.
pub(crate) fn ask_takeover(
    destination: &str,
    owner: &HelmOwnerRefusal,
    mark: &Mark,
    on_confirm: impl FnOnce(&mut Window, &mut App) + 'static,
    window: &mut Window,
    cx: &mut App,
) {
    let name = HostText::from_host(destination);
    let connected = if owner.connected {
        "That Farhelm is connected now.".to_string()
    } else {
        let now = crate::ticker::wall_now(cx)
            .duration_since(std::time::UNIX_EPOCH)
            .ok()
            .and_then(|duration| i64::try_from(duration.as_secs()).ok())
            .unwrap_or(owner.last_connected_at);
        let elapsed = now.saturating_sub(owner.last_connected_at).max(0);
        let (count, unit) = match elapsed {
            0..=59 => (0, "minute"),
            60..=3599 => (elapsed / 60, "minute"),
            3600..=86399 => (elapsed / 3600, "hour"),
            _ => (elapsed / 86400, "day"),
        };
        if count == 0 {
            "That Farhelm connected less than a minute ago.".to_string()
        } else {
            format!(
                "That Farhelm connected {count} {unit}{} ago.",
                if count == 1 { "" } else { "s" }
            )
        }
    };
    let summary = HostSentence::new("host-take-over-summary")
        .host(&name)
        .text(" is managed by another Farhelm (helm ")
        .mono(&HostText::from_host(owner.tag.as_str()))
        .text("). ")
        .text(connected);
    let confirm = Answer::new(
        "Take over",
        HoverHelp::tooltip("Manage this host and its sessions from this Farhelm"),
        Tier::Primary,
    );
    let question = Question::new("Take over host", confirm, ())
        .summary(summary)
        .line(concat!(
            "Taking over makes this Farhelm the host's manager. ",
            "The other Farhelm loses access to the host and its sessions until it takes it back."
        ))
        .width(px(WIDTH))
        .starts_on_cancel()
        .wait_until_shown();
    prompt::ask(
        question,
        None,
        mark,
        |(), window, cx| on_confirm(window, cx),
        window,
        cx,
    );
}

/// The open SSH or sandbox draft, its layer, and the operation key for its check.
pub(crate) struct AddHostDialog {
    /// The operation store's key for this opening's probe, and later its setup: fresh per opening, so a probe still out
    /// from an earlier opening never makes this one busy.
    key: OpKey,
    /// The control that opened the dialog, which gets focus back when it closes, also from the setup confirmation that
    /// takes its place, and when a refused setup reopens it.
    opener: FocusHandle,
    /// The mark taken at the last Add (at the opening until then): the setup confirmation opens only through it, so it
    /// never opens once the user has moved on since pressing Add ([`super::setup`]'s module docs).
    probe_mark: Mark,
    /// The SSH destination, exactly as typed.
    destination: DraftInput,
    /// The optional alias, as typed; blank means none.
    alias: DraftInput,
    /// The SSH route requested by this draft. Keep it attached to sign-in
    /// and to any confirmation that reopens the draft.
    tailscale: bool,
    /// The error line: why Add refused or what the probe answered. Cleared when a probe starts.
    problem: Option<HostSentence>,
    /// Set right after the layer opens; taken when the dialog closes itself, so it closes once.
    layer: Option<LayerId>,
    /// The agent's request the dialog answers, when a card opened it; kept after the helm accepted the dialog's Allow,
    /// for its labels.
    agent: Option<AgentRequest>,
    /// The helm accepted the dialog's Allow: from here on the add is ordinary and answers nothing more.
    accepted: bool,
    /// The add began as an agent's request: Add's approval lines show, and the setup confirmation always asks.
    from_agent: bool,
    /// What the dialog shows below its kind choice (the module docs, "The host kind").
    body: Body,
    /// Add and both Enter routes share the confirmation choice from the presented SSH form.
    setup_frame: crate::prompt::ActionFrame<bool, bool>,
    /// The question view and replay belong to the exact check from this dialog. Closing releases its operation hold.
    sign_in: Option<InlineSignIn>,
    _questions: session::ObservingCurrent,
    /// An agent's dialog's watches: a redraw when the answer bar's pause ends, which makes Add live again; and a close
    /// when an answer from another surface (the card) settles after the request stopped waiting, which settles in the
    /// operation store, not the pushed store the dialog's subject watches.
    _watches: Vec<Subscription>,
}

struct InlineSignIn {
    attempt: String,
    form: Option<(String, Entity<SignInForm>)>,
    waiting: Entity<setup_sign_in::Waiting>,
}

impl AddHostDialog {
    /// A dialog opened by `opener`, empty or as `seed` says. The drafts follow nothing: there is no pushed value behind
    /// a new host's fields.
    fn new(opener: FocusHandle, start: Start, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut sandbox = None;
        let (destination, alias, tailscale, problem, from_agent, agent) = match start {
            Start::Empty => (String::new(), String::new(), false, None, false, None),
            Start::Sandbox(seed) => {
                sandbox = Some(seed);
                (String::new(), String::new(), false, None, false, None)
            }
            Start::Seed(seed) => (
                seed.destination,
                seed.alias,
                seed.tailscale,
                Some(seed.problem),
                seed.from_agent,
                None,
            ),
            Start::Agent(request) => (
                request.destination.clone(),
                request.alias.clone(),
                false,
                None,
                true,
                Some(request),
            ),
        };
        let mut watches = Vec::new();
        if let Some(request) = agent.as_ref() {
            watches.push(cx.observe(&request.arming, |_, _, cx| cx.notify()));
            if let Some(operations) = session::current_operations(cx) {
                let handle = window.window_handle();
                watches.push(cx.observe(&operations, move |dialog: &mut Self, _, cx| {
                    if dialog.request_gone(cx) {
                        dialog.close_later(None, handle, cx);
                    }
                }));
            }
        }
        let destination_field = TextInputState::new(window, cx);
        let destination = DraftInput::with_submit(
            &destination,
            destination_field,
            |dialog: &mut Self| &mut dialog.destination,
            Self::submit_ssh_field,
            window,
            cx,
        );
        let alias_field = TextInputState::new(window, cx);
        let alias = DraftInput::with_submit(
            &alias,
            alias_field,
            |dialog: &mut Self| &mut dialog.alias,
            Self::submit_ssh_field,
            window,
            cx,
        );
        let key = OpKey::new_host_add();
        let handle = window.window_handle();
        let questions = session::observe_current(cx, move |_: &mut Self, cx| {
            Self::queue_question_refresh(handle, cx);
            cx.notify();
        });
        let body = match sandbox {
            Some(seed) => {
                let (dialog, key, opener) = (cx.weak_entity(), key.clone(), opener.clone());
                Body::Tensorlake(
                    cx.new(|cx| TensorlakeForm::new(dialog, key, opener, Some(seed), window, cx)),
                )
            }
            None => Body::Ssh,
        };
        AddHostDialog {
            key,
            opener,
            probe_mark: Mark::take(cx),
            destination,
            alias,
            tailscale,
            problem,
            layer: None,
            agent,
            accepted: false,
            from_agent,
            body,
            setup_frame: Default::default(),
            sign_in: None,
            _questions: questions,
            _watches: watches,
        }
    }

    /// The kind choice picked `kind`: show its body, a new Tensorlake sandbox form (which reads the account) or the
    /// SSH fields, dropping the sandbox's form. Ignored while Add's probe or answer is out, so the fields being checked
    /// stay on screen.
    fn choose_kind(&mut self, kind: Kind, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy(cx) || self.body.kind() == kind {
            return;
        }
        self.body = match kind {
            Kind::Ssh => Body::Ssh,
            Kind::Tensorlake => {
                let (dialog, key, opener) =
                    (cx.weak_entity(), self.key.clone(), self.opener.clone());
                Body::Tensorlake(
                    cx.new(|cx| TensorlakeForm::new(dialog, key, opener, None, window, cx)),
                )
            }
        };
        cx.notify();
    }

    /// The agent's request, while Add and Cancel still answer it: until the helm accepted the dialog's Allow.
    fn pending(&self) -> Option<&AgentRequest> {
        self.agent.as_ref().filter(|_| !self.accepted)
    }

    /// Whether the dialog's subject is gone: the pending request stopped waiting, with no answer to it out to say what
    /// came of it. The old dialog closed then, since an Add could only be refused.
    fn request_gone(&self, cx: &App) -> bool {
        self.pending()
            .is_some_and(|request| !request.answering(cx) && !request.waiting(cx))
    }

    /// Where focus starts as the dialog opens: the SSH destination, or a reopened sandbox's Name.
    fn start(&self, cx: &App) -> FocusHandle {
        match &self.body {
            Body::Ssh => self.destination.input().focus_handle(cx),
            Body::Tensorlake(form) => form.read(cx).name_handle(cx),
        }
    }

    /// The Tensorlake sandbox's recipe editor, while it is open in the shown body.
    fn recipe_editor(&self, cx: &App) -> Option<Entity<crate::settings::RecipeEditor>> {
        match &self.body {
            Body::Tensorlake(form) => form.read(cx).editor(),
            Body::Ssh => None,
        }
    }

    /// Cancel, the header's close or Escape. An open recipe editor answers first: while its save is out the dialog stays
    /// and asks nothing, and the close is not remembered, so the done save returns to the form (SPEC.md, "Settings":
    /// waiting for the helm's round trip is accepted); with unsaved changes it asks its leave question before Add host
    /// closes (the Tensorlake sandbox's module docs). A visible SSH question takes dismissal next so the user can cancel
    /// the SSH process and keep the draft. Otherwise, on an agent's add, answer Deny and close once the helm took it (or
    /// the request was already gone), keeping the dialog with the helm's reason when it refused; nothing happens while an
    /// answer is in flight or answers are paused, as for the card's own buttons. After an accepted Allow, or for an
    /// ordinary add, closing is ordinary.
    fn dismissal(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Dismissal {
        if let Some(editor) = self.recipe_editor(cx) {
            return editor.update(cx, |editor, cx| editor.dismissal(window, cx));
        }
        if let Some((_, form)) = self
            .sign_in
            .as_ref()
            .and_then(|sign_in| sign_in.form.as_ref())
        {
            form.update(cx, |form, cx| form.cancel(window, cx));
            return Dismissal::Stay;
        }
        let window = window.window_handle();
        if let Some(sign_in) = self.sign_in.take() {
            setup_sign_in::cancel_draft(&sign_in.attempt, cx);
        }
        let Some(request) = self.pending().cloned() else {
            return Dismissal::Close;
        };
        if request.answering(cx) {
            return Dismissal::Stay;
        }
        let dialog = cx.weak_entity();
        let sent = request.deny(cx, move |answered, cx| {
            let _ = dialog.update(cx, |dialog, cx| {
                match answered {
                    // A Deny is never unconfirmed (only an Allow needs the add's receipt); the arm keeps the match
                    // exhaustive.
                    Answered::Delivered | Answered::Gone | Answered::Unconfirmed => {
                        dialog.close_later(None, window, cx)
                    }
                    Answered::Refused(words) => {
                        dialog.problem = Some(error_line().host(&words));
                        cx.notify();
                    }
                }
            });
        });
        match sent {
            Ok(_) => {
                cx.notify();
                Dismissal::Stay
            }
            // Nothing was sent, so nothing waits on a reply: closing is safe, and the request keeps waiting on its
            // card.
            Err(not_started) => {
                tracing::debug!(?not_started, "agent host add Deny not sent");
                Dismissal::Close
            }
        }
    }

    /// Whether something Add started is out: this dialog's probe, or an answer to its agent's request. Add reads
    /// "Checking…" then.
    fn probing(&self, cx: &App) -> bool {
        self.pending().is_some_and(|request| request.answering(cx)) || key_busy(&self.key, cx)
    }

    /// Whether Add is busy: [`Self::probing`], or for an agent's request the answer bar's pause, during which Add keeps
    /// its label but answers nothing.
    fn busy(&self, cx: &App) -> bool {
        self.probing(cx) || self.pending().is_some_and(|request| request.paused(cx))
    }

    /// The dialog's main action, Enter on the dialog itself: the shown kind's. Add for SSH; the sandbox's Continue while
    /// its body shows, so Enter there never probes the SSH destination typed earlier and hidden now.
    fn main_action(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match &self.body {
            Body::Tensorlake(form) => form.clone().update(cx, |form, cx| form.proceed(window, cx)),
            Body::Ssh => self.add(window, cx),
        }
    }

    /// An agent's waiting request needs a fresh Enter. The press that opened the prefilled dialog can still be
    /// repeating when the destination gets focus, and opening the dialog is only a chance to review the request.
    fn submit_ssh_field(&mut self, held: bool, window: &mut Window, cx: &mut Context<Self>) {
        if held && self.pending().is_some() {
            return;
        }
        self.add(window, cx);
    }

    /// Add: refuse what can be refused without contacting anything, then refuse while a session launch is in flight,
    /// else start the probe, or for an agent's request not yet accepted, answer its Allow and probe once the helm took
    /// it. Does nothing while the probe or an answer is out, or the window is inert.
    fn add(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if session::current_operations(cx).is_none() || self.busy(cx) {
            return;
        }
        let Some(asks) = self
            .setup_frame
            .current()
            .and_then(|frame| frame.shown().copied())
        else {
            return;
        };
        let destination = self.destination.draft().value().clone();
        let alias = self.alias.draft().value().clone();
        if let Some(refused) = refusal(&destination, &alias, cx) {
            self.problem = Some(refused);
            cx.notify();
            return;
        }
        let handle = window.window_handle();
        // The same moment the operation store takes its own mark, which `Settled::user_moved_on` reports against. For
        // an agent's request it stays the press's mark through the Allow: the setup confirmation opens through it.
        self.probe_mark = Mark::take(cx);
        match self.pending().cloned() {
            Some(request) => self.allow(request, destination, alias, handle, cx),
            None => self.probe(destination, alias, asks, handle, cx),
        }
    }

    /// Answer the agent's request Allow with `destination`, holding the fields read-only meanwhile so the values on
    /// screen stay the ones approved; once the helm accepted it, the add is ordinary and probes what was approved,
    /// after the launch check again (a launch may have started while the answer was out). A late answer or a refusal
    /// shows in the error line and starts nothing (SPEC.md: "a late or refused answer starts no discovery probe").
    fn allow(
        &mut self,
        request: AgentRequest,
        destination: String,
        alias: String,
        handle: AnyWindowHandle,
        cx: &mut Context<Self>,
    ) {
        let dialog = cx.weak_entity();
        let approved = destination.clone();
        let sent = request.allow(approved, cx, move |answered, cx| {
            let _ = dialog.update(cx, |dialog, cx| {
                dialog.set_read_only(false, cx);
                match answered {
                    Answered::Delivered => {
                        dialog.accepted = true;
                        match refusal(&destination, &alias, cx) {
                            Some(refused) => dialog.problem = Some(refused),
                            None => dialog.probe(destination, alias, true, handle, cx),
                        }
                    }
                    // The request is gone, so the dialog has nothing left to approve: it closes, as the old one did
                    // once its request was gone, and the cards' late-answer notice says that nothing was added.
                    Answered::Gone => {
                        answers::post_late(false, cx);
                        dialog.close_later(None, handle, cx);
                    }
                    Answered::Refused(words) => dialog.problem = Some(error_line().host(&words)),
                    Answered::Unconfirmed => {
                        dialog.problem = Some(error_line().text(host_add::NOT_CONFIRMED));
                    }
                }
                cx.notify();
            });
        });
        match sent {
            Ok(host_add_sent) => {
                if host_add_sent == answers::Sent::Sent {
                    self.problem = None;
                    self.set_read_only(true, cx);
                }
                cx.notify();
            }
            Err(not_started) => tracing::debug!(?not_started, "agent host add Allow not sent"),
        }
    }

    /// Start the probe under the setup consent in effect when Add was pressed.
    fn probe(
        &mut self,
        destination: String,
        alias: String,
        asks_before_setup: bool,
        handle: AnyWindowHandle,
        cx: &mut Context<Self>,
    ) {
        let command = match probe_command(&destination, &alias, self.tailscale, self.from_agent) {
            Ok(command) => command,
            Err(error) => {
                tracing::debug!(?error, "host probe not built");
                return;
            }
        };
        self.request(command, asks_before_setup, handle, cx);
    }

    /// Probing and confirming use one operation slot. The fields stay read-only, and SSH sign-in replays the
    /// original probe with its original setup consent.
    fn request(
        &mut self,
        command: Command,
        asks_before_setup: bool,
        handle: AnyWindowHandle,
        cx: &mut Context<Self>,
    ) {
        let Some(operations) = session::current_operations(cx) else {
            return;
        };
        let confirming = matches!(command.work(), Work::TakeOverHost { registers: true });
        let replay = command.clone();
        let dialog = cx.weak_entity();
        let started = operations.update(cx, |operations, cx| {
            operations.start(&self.key, command, cx, move |settled, cx| {
                let pending = (!confirming)
                    .then(|| setup_sign_in::pending(&settled.outcome))
                    .flatten();
                let updated = dialog.update(cx, |dialog, cx| {
                    if let Some(pending) = &pending {
                        dialog.wait_for_setup(
                            pending.attempt.clone(),
                            replay,
                            asks_before_setup,
                            handle,
                            cx,
                        );
                    } else {
                        dialog.settled(settled, confirming, asks_before_setup, handle, cx);
                    }
                });
                if updated.is_err()
                    && let Some(pending) = pending
                {
                    // The pending reply can arrive after the dialog closes, before it knew what to cancel.
                    setup_sign_in::cancel_draft(&pending.attempt, cx);
                }
            })
        });
        match started {
            Ok(()) => {
                self.problem = None;
                self.set_read_only(true, cx);
                cx.notify();
            }
            Err(not_started) => tracing::debug!(?not_started, "host add request not sent"),
        }
    }

    /// Associate the 202 with this opening. Only a successful socket check lets the exact original probe resume.
    fn wait_for_setup(
        &mut self,
        attempt: String,
        replay: Command,
        asks_before_setup: bool,
        handle: AnyWindowHandle,
        cx: &mut Context<Self>,
    ) {
        if self.layer.is_none() {
            setup_sign_in::cancel_draft(&attempt, cx);
            return;
        }
        let dialog = cx.weak_entity();
        let waiting = setup_sign_in::Waiting::start(
            attempt.clone(),
            self.key.clone(),
            replay.work(),
            self.probe_mark,
            {
                let attempt = attempt.clone();
                move |result, mark, cx| {
                    let _ = dialog.update(cx, |dialog, cx| {
                        if !dialog
                            .sign_in
                            .as_ref()
                            .is_some_and(|current| current.attempt == attempt)
                        {
                            return;
                        }
                        dialog.sign_in = None;
                        dialog.probe_mark = mark;
                        dialog.set_read_only(false, cx);
                        match result {
                            Ok(()) => dialog.request(replay, asks_before_setup, handle, cx),
                            Err(reason) => dialog.problem = Some(error_line().host(&reason)),
                        }
                        cx.notify();
                    });
                }
            },
            cx,
        );
        let Some(waiting) = waiting else {
            setup_sign_in::cancel_draft(&attempt, cx);
            self.set_read_only(false, cx);
            self.problem = Some(
                error_line().text("SSH sign-in could not continue. Try adding the host again."),
            );
            cx.notify();
            return;
        };
        self.sign_in = Some(InlineSignIn {
            attempt,
            form: None,
            waiting,
        });
        // The question can already be in the store when the 202 arrives.
        Self::queue_question_refresh(handle, cx);
        cx.notify();
    }

    fn queue_question_refresh(handle: AnyWindowHandle, cx: &mut Context<Self>) {
        let dialog = cx.weak_entity();
        cx.defer(move |cx| {
            let _ = handle.update(cx, |_, window, cx| {
                let _ = dialog.update(cx, |dialog, cx| dialog.reconcile_question(window, cx));
            });
        });
    }

    /// Render only a question this draft owns. Long-running or multi-question sign-ins leave both drafts intact.
    fn reconcile_question(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(sign_in) = self.sign_in.as_mut() else {
            return;
        };
        let Some(store) = session::current_store(cx) else {
            return;
        };
        let question = store.read(cx).collections().ordered::<SshQuestion>().into_iter()
            .find(|question| matches!(&question.target, SshQuestionTarget::Draft { draft } if *draft == sign_in.attempt))
            .cloned();
        if sign_in.form.as_ref().map(|(id, _)| id) == question.as_ref().map(|question| &question.id)
        {
            return;
        }
        let attempt = sign_in.attempt.clone();
        let waiting = sign_in.waiting.downgrade();
        let dialog = cx.weak_entity();
        let handoff: PrepareHandoff = Rc::new(move |mark, window, cx| {
            let dialog = dialog.clone();
            let attempt = attempt.clone();
            let window = window.window_handle();
            let ready = waiting
                .upgrade()
                .map(|waiting| setup_sign_in::Waiting::answer_handoff(&waiting, mark, cx));
            Box::new(move |accepted, cx| {
                if let Some(ready) = ready {
                    ready(accepted, cx);
                }
                if !accepted {
                    return;
                }
                let _ = window.update(cx, |_, window, cx| {
                    let _ = dialog.update(cx, |dialog, cx| {
                        if let Some(sign_in) = dialog
                            .sign_in
                            .as_ref()
                            .filter(|sign_in| sign_in.attempt == attempt)
                        {
                            dialog.probe_mark = mark;
                            if let Some((_, form)) = &sign_in.form {
                                focus::focus_in_layer(
                                    &form.read(cx).first_stop(cx),
                                    &mark,
                                    window,
                                    cx,
                                );
                            }
                        }
                    });
                });
            })
        });
        sign_in.form = question.map(|question| {
            let id = question.id.clone();
            let form = cx.new(|cx| SignInForm::new(question, handoff, window, cx));
            focus::focus_in_layer(&form.read(cx).first_stop(cx), &self.probe_mark, window, cx);
            (id, form)
        });
        cx.notify();
    }

    /// The probe settled: free the fields, then close the dialog for a registered host, show what the helm said, or
    /// go on to setup in one of the three ways [`super::setup`]'s module docs describe.
    fn settled(
        &mut self,
        settled: &Settled,
        confirming: bool,
        asks_before_setup: bool,
        window: AnyWindowHandle,
        cx: &mut Context<Self>,
    ) {
        self.set_read_only(false, cx);
        let reply = read_reply(&settled.outcome);
        if confirming && !matches!(reply, Reply::Added { .. } | Reply::Show(_)) {
            self.problem =
                Some(error_line().text("Farhelm could not confirm that it added the host."));
            cx.notify();
            return;
        }
        match reply {
            Reply::Added { host_id } => {
                self.close_later(None, window, cx);
                let mark = self.probe_mark;
                cx.defer(move |cx| {
                    let _ = window.update(cx, |_, window, cx| {
                        crate::sign_in::started(host_id, mark, window, cx);
                    });
                });
            }
            Reply::Show(problem) => self.problem = Some(problem.sentence(error_line())),
            Reply::TakeOver { owner, .. } if self.from_agent => {
                self.problem = Some(ownership_problem(&owner).sentence(error_line()));
            }
            Reply::TakeOver { probe_id, owner } => {
                let mark = self.probe_mark;
                if focus::has_moved_on(&mark, cx) {
                    self.problem = Some(ownership_problem(&owner).sentence(error_line()));
                } else {
                    let destination = self.destination.draft().value().clone();
                    let dialog = cx.weak_entity();
                    cx.defer(move |cx| {
                        let _ = window.update(cx, |_, window, cx| {
                            if dialog.upgrade().is_none() {
                                return;
                            }
                            ask_takeover(
                                &destination,
                                &owner,
                                &mark,
                                move |window, cx| {
                                    let _ = dialog.update(cx, |dialog, cx| {
                                        dialog.confirm_takeover(
                                            probe_id,
                                            window.window_handle(),
                                            cx,
                                        );
                                    });
                                },
                                window,
                                cx,
                            );
                        });
                    });
                }
            }
            Reply::Setup {
                probe_id,
                confirmation,
                steps,
            } => {
                let mark = self.probe_mark;
                let offer = setup::Offer {
                    key: self.key.clone(),
                    probe_id,
                    confirmation,
                    steps,
                    target: setup::Target::New {
                        destination: self.destination.draft().value().clone(),
                        alias: self.alias.draft().value().clone(),
                        tailscale: self.tailscale,
                        opener: self.opener.clone(),
                        from_agent: self.from_agent,
                    },
                    // The setup's own action replaces it when the user answers the confirmation; without asking, the
                    // plan goes out on the strength of the Add, so a refusal is judged against that.
                    mark,
                };
                // An agent's Add always asks. For a manual Add, the user can restore confirmation during the probe.
                if !self.from_agent && !asks_before_setup && !setup::asks_first(cx) {
                    // The close is queued first: a submission refused at once (a launch in flight) reopens Add host
                    // on a later turn, which must come after this dialog's close, or the close would take the
                    // reopened dialog with it (closing a layer closes the layers opened from it, and the reopened
                    // dialog would be one).
                    self.close_later(None, window, cx);
                    setup::submit(offer, window, cx);
                } else if focus::has_moved_on(&mark, cx) {
                    self.problem = Some(error_line().text(setup::MOVED_ON));
                } else {
                    self.close_later(Some(offer), window, cx);
                }
            }
        }
        cx.notify();
    }

    /// Confirm the probe shown by the question. The helm owns the id and
    /// checks the retained discovery before registering anything.
    fn confirm_takeover(
        &mut self,
        probe_id: String,
        window: AnyWindowHandle,
        cx: &mut Context<Self>,
    ) {
        if super::launch_in_flight(cx) {
            self.problem = Some(error_line().text(super::LAUNCH_IN_FLIGHT));
            cx.notify();
            return;
        }
        self.probe_mark = Mark::take(cx);
        let Ok(path) = ApiPath::api(&["hosts", "take-over"]) else {
            return;
        };
        self.request(
            Command::new(
                Work::TakeOverHost { registers: true },
                Method::Post,
                path,
                Some(json!({ "probe_id": probe_id })),
                "POST /api/hosts/take-over",
            ),
            true,
            window,
            cx,
        );
    }

    /// The destination field's state, for a golden scene to put text in, since a scene cannot type. Setting it leaves
    /// the draft as it was, which no picture shows.
    #[cfg(test)]
    pub(crate) fn destination_input(&self) -> TextInputState {
        self.destination.input().clone()
    }

    /// Render a completed probe in the unchanged Add dialog. The golden supplies the helm's reply so the scene
    /// exercises the live error presentation without connecting to a host.
    #[cfg(test)]
    pub(crate) fn show_probe_reply_for_golden(
        &mut self,
        tailscale: bool,
        outcome: Outcome,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.tailscale = tailscale;
        let settled = Settled {
            outcome,
            user_moved_on: false,
        };
        self.settled(&settled, false, true, window.window_handle(), cx);
    }

    /// Show a checking draft matched to the fixture's pending question. The golden does not send terminal input
    /// or run the probe again. The Add dialog tests cover those actions through the real interaction.
    #[cfg(test)]
    pub(crate) fn wait_for_setup_for_golden(
        &mut self,
        attempt: &str,
        destination: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Ok(command) = probe_command(destination, "", self.tailscale, false) else {
            return;
        };
        self.set_read_only(true, cx);
        self.wait_for_setup(attempt.into(), command, true, window.window_handle(), cx);
    }

    /// Hold both fields read-only while the probe is out, so the values on screen stay the ones sent.
    fn set_read_only(&self, read_only: bool, cx: &mut App) {
        self.destination.input().set_read_only(read_only, cx);
        self.alias.input().set_read_only(read_only, cx);
    }

    /// Close the dialog from the probe's completion, which has no window at hand: once the completion has returned, in
    /// `window` if it is still open. With `confirm`, then ask the setup confirmation for it in the dialog's place,
    /// through its mark. Closed first: closing a layer also closes the layers opened from it, and focus goes back to
    /// the opener before the question records where to return it.
    fn close_later(
        &mut self,
        confirm: Option<setup::Offer>,
        window: AnyWindowHandle,
        cx: &mut Context<Self>,
    ) {
        let Some(layer) = self.layer.take() else {
            return;
        };
        cx.defer(move |cx| {
            let _ = window.update(cx, |_, window, cx| {
                layers::close(layer, window, cx);
                if let Some(offer) = confirm {
                    setup::confirm(offer, window, cx);
                }
            });
        });
    }
}

/// Whether work is out under an Add host opening's `key`: its probe, or the setup or sandbox creation it led to. Read
/// by the dialog and by its Tensorlake sandbox form, which runs under the same key and cannot read the dialog while
/// the dialog draws it.
fn key_busy(key: &OpKey, cx: &App) -> bool {
    session::current_operations(cx)
        .is_some_and(|operations| operations.read(cx).in_flight(key).is_some())
}

/// A click handler that dismisses the dialog through the layer manager, as Escape does: what Cancel and the header's
/// close control do, probe or no probe. Called from the click, not from inside an update of the dialog.
fn dismiss(
    dialog: WeakEntity<AddHostDialog>,
) -> impl Fn(&ClickEvent, &mut Window, &mut App) + 'static {
    move |_, window, cx| {
        let layer = dialog.upgrade().and_then(|dialog| dialog.read(cx).layer);
        if let Some(layer) = layer {
            layers::dismiss(layer, window, cx);
        }
    }
}

/// A labelled field (`.add-host-form label`): the label's words in the quiet ink over the control, a text input or the
/// sandbox's Recipe select. The ink is the words' own, not the column's: the control inherits text color, and its text
/// is the ordinary ink (`.add-host-form input`'s `--fg-0`).
fn field(label: &'static str, input: impl IntoElement, palette: &theme::Palette) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap(px(LABEL_GAP))
        .child(
            div()
                .text_size(metrics::TEXT_SM)
                .text_color(theme::color(palette.fg_1))
                .child(label),
        )
        .child(div().text_color(theme::color(palette.fg_0)).child(input))
        .into_any_element()
}

/// A field's help (`.host-dialog-help`): 12-pixel text in the quiet ink at a line height of 1.5.
fn help(words: &'static str, palette: &theme::Palette) -> AnyElement {
    div()
        .text_size(metrics::TEXT_SM)
        .line_height(px(18.))
        .text_color(theme::color(palette.fg_1))
        .child(words)
        .into_any_element()
}

impl Render for AddHostDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = theme::palette(cx);
        let probing = self.probing(cx);
        let busy = self.busy(cx);
        if matches!(self.body, Body::Ssh) {
            self.setup_frame
                .capture(self.from_agent || setup::asks_first(cx), busy, window);
        } else {
            self.setup_frame.clear();
        }
        let weak = cx.weak_entity();
        // An agent's add names who asked, then spells out what Add approves, above the fields as the old dialog did
        // (the approvals module draws both).
        let requester = self.agent.as_ref().map(|request| request.requester(cx));
        let approved = self.from_agent.then(|| {
            host_add::approved_fields(
                self.destination.draft().value(),
                self.alias.draft().value(),
                cx,
            )
        });
        // The kind choice, above whatever the chosen kind draws. An add that began as an agent's request (its reopen
        // after a refused setup included) asks for an SSH destination, so it has none, as in the old dialog; and the
        // recipe editor, while open, hides it, as the old one replaced the whole body.
        let editor = self.recipe_editor(cx);
        let kinds = (!self.from_agent && editor.is_none()).then(|| {
            let chooser = weak.clone();
            TabStrip::new(
                "add-host-kind",
                "Host kind",
                HoverHelp::tooltip(
                    "Host kind: a machine you reach over SSH, or a new Tensorlake sandbox",
                ),
            )
            .tab(TabOption::new(
                "SSH",
                HoverHelp::tooltip("Connect a machine through its SSH destination"),
                Kind::Ssh,
            ))
            .tab(TabOption::new(
                "Tensorlake sandbox",
                HoverHelp::tooltip("Create a Tensorlake sandbox from a saved recipe"),
                Kind::Tensorlake,
            ))
            .chosen(self.body.kind())
            .on_choose(move |kind, window, cx| {
                let _ = chooser.update(cx, |dialog, cx| dialog.choose_kind(*kind, window, cx));
            })
        });
        let shown: AnyElement = match &self.body {
            Body::Tensorlake(form) => form.clone().into_any_element(),
            Body::Ssh => self.ssh_fields(palette, busy, cx),
        };
        let body = div()
            .flex()
            .flex_col()
            .gap(px(BODY_GAP))
            .children(requester)
            .children(approved)
            .children(kinds)
            .child(shown);
        let cancel = Button::new(
            "add-host-cancel",
            "Cancel",
            HoverHelp::tooltip(CANCEL_HELP),
            Role::Button,
        )
        .tier(Tier::Neutral)
        .on_click(dismiss(weak.clone()));
        let answering = self
            .sign_in
            .as_ref()
            .is_some_and(|sign_in| sign_in.form.is_some());
        let add_weak = weak.clone();
        let (main, summary) = match &self.body {
            Body::Tensorlake(form) => (
                form.read(cx).continue_button(form.downgrade(), cx),
                "A Tensorlake sandbox created from a recipe",
            ),
            Body::Ssh => (
                Button::new(
                    "add-host-submit",
                    if probing { "Checking…" } else { "Add" },
                    HoverHelp::tooltip("Add: check the SSH destination, then add the host"),
                    Role::Button,
                )
                .tier(Tier::Primary)
                // Busy rather than disabled while the probe is out, so the Add the user pressed keeps focus
                // (SPEC_impl.md, "Drafts": a disabled button gives up focus).
                .busy(busy)
                .on_click(move |_, window, cx| {
                    let _ = add_weak.update(cx, |dialog, cx| dialog.add(window, cx));
                }),
                "An SSH destination this helm can reach",
            ),
        };
        // The recipe editor's answers take the footer while it is open, as the old dialog's did.
        let frame = match editor {
            Some(editor) => DialogFrame::new("add-host", "Add host", body, Vec::new())
                .notices(crate::settings::RecipeEditor::actions(&editor, cx)),
            None if answering => DialogFrame::new("add-host", "Add host", body, Vec::new()),
            None => DialogFrame::new("add-host", "Add host", body, vec![cancel, main]),
        };
        frame
            .summary(summary)
            .tile(host_mark(
                HostIcon::Cloud,
                HostColor::Default,
                px(TILE_MARK),
                palette,
            ))
            .width(px(WIDTH))
            .close(
                if answering {
                    "Cancel SSH sign-in"
                } else {
                    "Cancel adding host"
                },
                HoverHelp::tooltip(if answering {
                    "Cancel this SSH sign-in and keep the draft"
                } else {
                    CANCEL_HELP
                }),
                false,
                dismiss(weak),
            )
    }
}

impl AddHostDialog {
    /// The draft and its current setup question. While checking or signing in,
    /// the controls show the same route as the retained probe.
    fn ssh_fields(&self, palette: &theme::Palette, busy: bool, cx: &Context<Self>) -> AnyElement {
        const TAILSCALE_HELP: &str = "Verify the host's SSH key through Tailscale instead of ~/.ssh/known_hosts, and resolve Tailscale node names.";
        let dialog = cx.weak_entity();
        div()
            .flex()
            .flex_col()
            .gap(px(BODY_GAP))
            .child(field(
                "SSH destination",
                TextInput::new(
                    "add-host-destination",
                    self.destination.input(),
                    "SSH destination",
                    HoverHelp::tooltip("user@host, a Tailscale node name, or a Host name from your ssh config"),
                )
                .monospace()
                .height(px(FIELD_HEIGHT)),
                palette,
            ))
            .child(help(
                "user@host, a Tailscale node name, or a Host name from your ssh config. Farhelm checks that it can reach the destination \
                 before adding it.",
                palette,
            ))
            .child(div().flex().flex_col().gap(px(LABEL_GAP))
                .child(Checkbox::new(
                    "add-host-tailscale", "Tailscale host", HoverHelp::tooltip(TAILSCALE_HELP), Role::CheckBox,
                )
                .checked(self.tailscale)
                .disabled(busy)
                .on_toggle(move |on, _, cx| {
                    let _ = dialog.update(cx, |dialog, cx| {
                        if !dialog.busy(cx) {
                            dialog.tailscale = on;
                            cx.notify();
                        }
                    });
                }))
                .child(help(TAILSCALE_HELP, palette)))
            .children(self.sign_in.as_ref().and_then(|sign_in| sign_in.form.as_ref()).map(|(_, form)| {
                div().flex().flex_col().gap(px(LABEL_GAP))
                    .child("Sign in for checking and setting up")
                    .child(help("Farhelm signs in once to check and set up the host, then again for sessions.", palette))
                    .child(form.clone())
            }))
            .child(field(
                "Alias (optional)",
                TextInput::new(
                    "add-host-alias",
                    self.alias.input(),
                    "Alias (optional)",
                    HoverHelp::tooltip(
                        "Alias: shown instead of the destination everywhere except the host's details",
                    ),
                )
                .height(px(FIELD_HEIGHT)),
                palette,
            ))
            .child(help(
                "Shown instead of the destination everywhere except the host's details. You can change it later in \
                 the host's settings.",
                palette,
            ))
            // The helm's words, which may quote the destination or the host's own text, so they are drawn as host
            // text, as one labelled node.
            .children(
                self.problem
                    .clone()
                    .map(|line| super::problem_line(line, palette)),
            )
            .into_any_element()
    }
}
