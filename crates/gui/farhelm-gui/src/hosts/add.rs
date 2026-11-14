//! The Add host dialog: an SSH destination and an optional alias, checked by the helm's probe before anything is
//! added.
//!
//! SPEC.md, "Topology" (the add host dialog, the host registry and its duplicate refusal). Ported from the old app's
//! `hosts.rs` (`AddHostForm`, `validate_alias_draft`) and `api.rs` (`probe_ssh_host`), with its look from the
//! `.host-add-dialog`, `.add-host-form` and `.host-dialog-help` rules in `app.css`, all at reference commit `24bc0580`.
//! Here's the TLDR of its rules:
//!
//! - **Opening.** [`open`] is the entry point for the host rail's Add host and the Hosts panel heading's add button.
//!   The opener takes focus first and gets it back when the dialog closes, as Settings' gear does. Focus starts in the
//!   destination field.
//! - **Before the probe is sent.** Add refuses an alias the helm would refuse for its shape (the old app's
//!   `validate_alias_draft`, in its words) and a destination that is already a host entry, naming that entry in the
//!   helm's own words (SPEC.md: "Adding a destination that is already a host entry is refused, naming that entry,
//!   before anything contacts the destination"). The helm refuses both before it contacts anything too, so the app's
//!   checks are not what keeps the destination from being dialed. They are there to answer at once: the helm runs a
//!   probe's checks only once the probe holds one of its few planning slots, which other hosts' probes may be holding,
//!   so its refusal can come only after them, or as the timeout reply. The duplicate check reads the pushed hosts and
//!   compares the destination exactly as typed, as the helm does; the helm checks both again, so a host added elsewhere
//!   a moment ago is still refused, by the helm's reply instead.
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
//!   correction. A destination with no supervisor (`provisionable`) is where the setup confirmation will open; see the
//!   NOTE below.
//!
//! An outcome shows in the dialog whether or not the user moved on since Add: drawing a line inside the dialog that is
//! already open moves no focus and opens nothing, which is all SPEC.md's "Keyboard focus" rule withholds from a late
//! result. `discovered` closes the dialog either way, since the dialog is its own subject (the rule
//! [`crate::operations::Settled::user_moved_on`] documents for Rename's success), and the layer manager returns focus
//! only if it is still inside. If the user closed the dialog before the reply, the reply reaches nobody.
//!
//! NOTE: Interim, until the setup confirmation is ported. A `provisionable` reply says in the error line that this
//! build cannot set up a host yet, and the helm's plan is dropped with the reply: nothing outside this completion holds
//! it. The setup confirmation, opened only through the moved-on mark the operation store took at Add, replaces that
//! line.
//!
//! NOTE: Interim, until the launcher starts its launches in the operation store (under `OpKey::new_launch()`, as a
//! launch kind of work): Add is not refused while a launch is in flight, as the old app refused it. Each side is to
//! refuse its own action, with a reason, while the other's work is in flight in the operation store (decided by the
//! migration's orchestrator on 2026-11-14); once launches are there, Add's refusal is one `any_in_flight` check.
//!
//! NOTE: Not drawn, each recorded in `docs/gpui/parity/settings-and-hosts.md`: the host kind choice (SSH or Tensorlake
//! sandbox), since this milestone adds hosts over SSH only, and the "Does this host ask for a second factor ...?" line
//! with its link to SSH help, which arrives with SSH help.

#[cfg(test)]
pub(crate) mod tests;

use std::rc::Rc;

use farhelm_helm_client::{ApiPath, Method, PathError};
use farhelm_proto::host_appearance::{HostColor, HostIcon};
use farhelm_proto::provisioning::ProbeReply;
use farhelm_proto::ui_channel::HostView;
use gpui::{
    AnyElement, AnyWindowHandle, App, AppContext as _, ClickEvent, Context, FocusHandle,
    IntoElement, ParentElement as _, Render, Styled as _, WeakEntity, Window, div, px,
};
use serde::Deserialize as _;
use serde::de::IgnoredAny;
use serde_json::{Value, json};

use crate::components::{Button, DialogFrame, HoverHelp, Role, TextInput, TextInputState, Tier};
use crate::drafts::DraftInput;
use crate::focus::{self, Mark, ReturnTo};
use crate::host_text::{HostSentence, HostText};
use crate::layers::{self, Dialog, LayerId, MainAction};
use crate::operations::{Command, OpKey, Outcome, Settled, Work};
use crate::session;
use crate::session_list::host_rail::{display_name, host_mark};
use crate::theme::{self, metrics};

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

/// Open the Add host dialog on top of everything else in `window`: the entry point for the host rail's Add host and
/// the Hosts panel heading's add button.
///
/// `opener` is the clicked control's focus handle and `mark` the [`Mark`] taken in its click handler. The opener takes
/// focus first, so that closing the dialog returns focus to it on every platform, also where a click leaves a button
/// unfocused (macOS), as [`crate::settings::open`] does for the gear (SPEC.md: "Closing something the user opened
/// returns focus to where it was before it opened, on every platform"). Nothing opens, and the opener takes no focus,
/// when the user has moved on since `mark` or the window is inert.
///
/// Returns the dialog's layer, or `None` when nothing opened.
pub fn open(
    opener: &FocusHandle,
    mark: &Mark,
    window: &mut Window,
    cx: &mut App,
) -> Option<LayerId> {
    // An inert window has no hosts to check against and nothing to probe through.
    session::current_store(cx)?;
    if focus::has_moved_on(mark, cx) {
        return None;
    }
    focus::focus_control(opener, mark, window, cx);
    let dialog = cx.new(|cx| AddHostDialog::new(window, cx));
    let start = dialog.read(cx).destination.input().focus_handle(cx);
    let weak = dialog.downgrade();
    // Enter on the dialog itself (after a click on its empty space) is Add, the dialog's one main action.
    let main_action: MainAction = Rc::new(move |window, cx| {
        let _ = weak.update(cx, |dialog, cx| dialog.add(window, cx));
    });
    let layer = layers::open_dialog(
        Dialog {
            view: dialog.clone().into(),
            container: cx.focus_handle(),
            start,
            subject: None,
            main_action: Some(main_action),
            // SPEC.md, "Opening and closing": the dialogs that share the host and Settings installer keep focus on the
            // control in use when their empty space is clicked.
            keeps_focus_on_empty_click: true,
            returns_to: ReturnTo::Opener,
            // None: closing never waits, not even for a probe in flight (the module docs).
            on_dismiss: None,
            placement: layers::DialogPlacement::Centered,
            dismiss_on_outside_press: false,
        },
        mark,
        window,
        cx,
    )?;
    dialog.update(cx, |dialog, _| dialog.layer = Some(layer));
    #[cfg(test)]
    cx.set_global(tests::Opened(dialog.downgrade()));
    Some(layer)
}

/// `POST /api/hosts/probe` for `destination` as a new host entry under `alias`: the destination exactly as typed, and
/// the alias as typed, or `null` when it is blank, which the helm reads as no alias (it trims an alias itself). No
/// install paths: the probe looks where setup installs, as it did for the old dialog, which stopped asking for them.
pub(crate) fn probe_command(destination: &str, alias: &str) -> Result<Command, PathError> {
    let alias = (!alias.trim().is_empty()).then_some(alias);
    Ok(Command::new(
        Work::Probe,
        Method::Post,
        ApiPath::api(&["hosts", "probe"])?,
        Some(json!({
            "target": { "kind": "ssh", "destination": destination },
            "new_entry": { "alias": alias },
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

/// The dialog's error line, empty: what every outcome it shows starts from.
fn error_line() -> HostSentence {
    HostSentence::new("add-host-error")
}

/// What the dialog shows for a probe's reply: `None` when the host was registered (the dialog closes instead), else
/// the line to show. The probe never answers 202, but an accepted reply is read the same way as a done one, by its
/// body.
fn outcome_line(outcome: &Outcome) -> Option<HostSentence> {
    let body = match outcome {
        Outcome::Refused(words) => return Some(error_line().host(words)),
        Outcome::Done(body) | Outcome::Accepted(body) => body.as_ref().unwrap_or(&Value::Null),
    };
    // The plan is the helm's own and never read here (`ProbeReply`'s docs); ignoring it also drops it with the reply.
    match ProbeReply::<IgnoredAny>::deserialize(body) {
        Ok(ProbeReply::Discovered { .. }) => None,
        Ok(ProbeReply::Manual { reason }) => {
            Some(error_line().host(&HostText::from_host(reason.as_str())))
        }
        Ok(ProbeReply::Provisionable { .. }) => Some(error_line().text(
            "Farhelm is not running on this host, and this version of the app cannot set it up yet; nothing was \
             added",
        )),
        // The old app's words for the same impossible reply to an SSH probe.
        Ok(ProbeReply::TensorlakeProvisionable { .. }) => {
            Some(error_line().text("The helm returned a Tensorlake plan for an SSH host"))
        }
        // The decoder's words quote the reply (an unknown variant's name, a string it did not expect), so they are host
        // text.
        Err(error) => Some(
            error_line()
                .text("The helm accepted the host probe, but its reply could not be read: ")
                .host(&HostText::from_host(error.to_string())),
        ),
    }
}

/// The open dialog: its two fields, what it last had to say, the key its probe runs under, and its layer.
pub(crate) struct AddHostDialog {
    /// The operation store's key for this opening's probe, and later its setup: fresh per opening, so a probe still out
    /// from an earlier opening never makes this one busy.
    key: OpKey,
    /// The SSH destination, exactly as typed.
    destination: DraftInput,
    /// The optional alias, as typed; blank means none.
    alias: DraftInput,
    /// The error line: why Add refused or what the probe answered. Cleared when a probe starts.
    problem: Option<HostSentence>,
    /// Set right after the layer opens; taken when the dialog closes itself, so it closes once.
    layer: Option<LayerId>,
}

impl AddHostDialog {
    /// An empty dialog. The drafts follow nothing: there is no pushed value behind a new host's fields.
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let destination = TextInputState::new(window, cx);
        let destination = DraftInput::new(
            "",
            destination,
            |dialog: &mut Self| &mut dialog.destination,
            Self::add,
            window,
            cx,
        );
        let alias = TextInputState::new(window, cx);
        let alias = DraftInput::new(
            "",
            alias,
            |dialog: &mut Self| &mut dialog.alias,
            Self::add,
            window,
            cx,
        );
        AddHostDialog {
            key: OpKey::new_host_add(),
            destination,
            alias,
            problem: None,
            layer: None,
        }
    }

    /// Whether this dialog's probe is out.
    fn probing(&self, cx: &App) -> bool {
        session::current_operations(cx)
            .is_some_and(|operations| operations.read(cx).in_flight(&self.key).is_some())
    }

    /// Add: refuse what can be refused without contacting anything, else start the probe. Does nothing while the probe
    /// is out or the window is inert.
    fn add(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(operations) = session::current_operations(cx) else {
            return;
        };
        if operations.read(cx).in_flight(&self.key).is_some() {
            return;
        }
        let destination = self.destination.draft().value().clone();
        let alias = self.alias.draft().value().clone();
        let refused = session::current_store(cx).and_then(|store| {
            refused_before_contact(
                &destination,
                &alias,
                store.read(cx).collections().ordered::<HostView>(),
            )
        });
        if let Some(refused) = refused {
            self.problem = Some(refused);
            cx.notify();
            return;
        }
        let command = match probe_command(&destination, &alias) {
            Ok(command) => command,
            Err(error) => {
                tracing::debug!(?error, "host probe not built");
                return;
            }
        };
        let handle = window.window_handle();
        let dialog = cx.weak_entity();
        let started = operations.update(cx, |operations, cx| {
            operations.start(&self.key, command, cx, move |settled, cx| {
                let _ = dialog.update(cx, |dialog, cx| dialog.settled(settled, handle, cx));
            })
        });
        match started {
            Ok(()) => {
                self.problem = None;
                self.set_read_only(true, cx);
                cx.notify();
            }
            Err(not_started) => tracing::debug!(?not_started, "host probe not sent"),
        }
    }

    /// The probe settled: free the fields, then close the dialog for a registered host or show what the helm said.
    fn settled(&mut self, settled: &Settled, window: AnyWindowHandle, cx: &mut Context<Self>) {
        self.set_read_only(false, cx);
        match outcome_line(&settled.outcome) {
            None => self.close_later(window, cx),
            Some(line) => self.problem = Some(line),
        }
        cx.notify();
    }

    /// The destination field's state, for a golden scene to put text in, since a scene cannot type. Setting it leaves
    /// the draft as it was, which no picture shows.
    #[cfg(test)]
    pub(crate) fn destination_input(&self) -> TextInputState {
        self.destination.input().clone()
    }

    /// Hold both fields read-only while the probe is out, so the values on screen stay the ones sent.
    fn set_read_only(&self, read_only: bool, cx: &mut App) {
        self.destination.input().set_read_only(read_only, cx);
        self.alias.input().set_read_only(read_only, cx);
    }

    /// Close the dialog from the probe's completion, which has no window at hand: once the completion has returned, in
    /// `window` if it is still open.
    fn close_later(&mut self, window: AnyWindowHandle, cx: &mut Context<Self>) {
        let Some(layer) = self.layer.take() else {
            return;
        };
        cx.defer(move |cx| {
            let _ = window.update(cx, |_, window, cx| layers::close(layer, window, cx));
        });
    }
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

/// A labelled field (`.add-host-form label`): the label's words in the quiet ink over the input. The ink is the
/// words' own, not the column's: the input inherits text color, and its text is the ordinary ink (`.add-host-form
/// input`'s `--fg-0`).
fn field(label: &'static str, input: TextInput, palette: &theme::Palette) -> AnyElement {
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
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = theme::palette(cx);
        let probing = self.probing(cx);
        let weak = cx.weak_entity();
        let body = div()
            .flex()
            .flex_col()
            .gap(px(BODY_GAP))
            .child(field(
                "SSH destination",
                TextInput::new(
                    "add-host-destination",
                    self.destination.input(),
                    "SSH destination",
                    HoverHelp::tooltip("SSH destination: user@host, or a Host name from your ssh config"),
                )
                .monospace()
                .height(px(FIELD_HEIGHT)),
                palette,
            ))
            .child(help(
                "user@host, or a Host name from your ssh config. Farhelm checks that it can reach the destination \
                 before adding it.",
                palette,
            ))
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
            .children(self.problem.clone().map(|line| {
                // `.create-session-error`: the helm's words, which may quote the destination or the host's own text,
                // so they are drawn as host text, as one labelled node.
                div()
                    .pt(px(2.))
                    .px(px(14.))
                    .pb(px(8.))
                    .text_size(metrics::TEXT_SM)
                    .text_color(theme::color(palette.danger))
                    .child(line)
            }));
        let cancel = Button::new(
            "add-host-cancel",
            "Cancel",
            HoverHelp::tooltip(CANCEL_HELP),
            Role::Button,
        )
        .tier(Tier::Neutral)
        .on_click(dismiss(weak.clone()));
        let add_weak = weak.clone();
        let add = Button::new(
            "add-host-submit",
            if probing { "Checking…" } else { "Add" },
            HoverHelp::tooltip("Add: check the SSH destination, then add the host"),
            Role::Button,
        )
        .tier(Tier::Primary)
        // Busy rather than disabled while the probe is out, so the Add the user pressed keeps focus (SPEC_impl.md,
        // "Drafts": a disabled button gives up focus).
        .busy(probing)
        .on_click(move |_, window, cx| {
            let _ = add_weak.update(cx, |dialog, cx| dialog.add(window, cx));
        });
        DialogFrame::new("add-host", "Add host", body, vec![cancel, add])
            .summary("An SSH destination this helm can reach")
            .tile(host_mark(
                HostIcon::Cloud,
                HostColor::Default,
                px(TILE_MARK),
                palette,
            ))
            .width(px(WIDTH))
            .close(
                "Cancel adding host",
                HoverHelp::tooltip(CANCEL_HELP),
                false,
                dismiss(weak),
            )
    }
}
