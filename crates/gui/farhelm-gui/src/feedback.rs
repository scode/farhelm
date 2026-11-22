//! The Send feedback dialog: a private message to Farhelm's maintainer, sent through the helm.
//!
//! SPEC.md, "Sending feedback", and SPEC_impl.md, "Feedback forwarding". Ported from the old app's `feedback.rs`
//! (`FeedbackDialog`, `may_close`, `contact_after_success`) and the `.feedback-*` rules in `app.css` at reference
//! commit `24bc0580`. The dialog's promise is that what it shows is the whole submission and that nothing leaves the
//! machine until Send. Here's the TLDR of how it keeps that promise:
//!
//! - **What is shown is what is sent.** The version, that this is the desktop app, and the operating system are each
//!   computed once when the dialog opens, fitted to the shared caps ([`fit_machine_field`]) and stored, so the value on
//!   screen and the value in the submission are the same string. The message and the contact go as typed: nothing is
//!   trimmed, and a blank contact goes as no contact, the one wire spelling for it.
//! - **Plain fields, not drafts.** The message and the contact are ordinary inputs. Only the contact's first value
//!   comes from the store (the remembered contact in the pushed preferences, read as the dialog opens); nothing pushed
//!   later overwrites what the user types.
//! - **Nothing goes out before Send.** Send posts `POST /api/feedback` to the helm as work on the feedback key
//!   ([`OpKey::feedback`]), and the helm forwards it; the window never talks to the endpoint. Whether a send is out is
//!   the operation store's to say ([`sending`]); the dialog keeps no copy of it. While one is out, Send draws busy, the
//!   fields are read-only, Cancel and the header Close are disabled, and every dismissal stays ([`may_close`], asked by
//!   the layer manager's dismissal hook), so the user cannot lose the send's outcome or their text.
//! - **Success thanks and closes; failure keeps the text.** On success the body becomes a thank-you line (a stop of
//!   its own, "Feedback sent", so the keyboard can scroll it), focus moves to the footer's Close (the fields it may
//!   have held are gone), and the dialog closes itself after
//!   [`THANKS_SHOWN`]. On any refusal the dialog says sending failed in plain words ([`failure_words`]), keeps
//!   everything typed, and lets the user retry or copy it. Nothing is queued or retried. Focus that a click on Send left
//!   on the dialog itself (macOS) goes back to the message, so the user can type on, unless the user moved on; focus
//!   on a control stays.
//! - **Remembering the contact is a separate write.** Only after a successful send, a best-effort keyless
//!   `PUT /api/preferences` writes exactly the contact sent, or clears it when Re-use was unchecked or the field was
//!   blank ([`contact_after_success`]). It writes even when the value looks unchanged here, since the helm's copy may
//!   have changed since. A failed send writes nothing.
//!
//! Agents have no way to open or send this dialog: there is no agent request or command line verb for it, and none may
//! be added (SPEC.md: feedback sent that way would leave the machine without the user pressing Send).
//!
//! [`open`] is the entry point: the Help menu's Send feedback item calls it with the `?` button as the opener. The
//! Welcome card's Send feedback now calls [`open_returning`] with its own return target and, for the card that opened
//! by itself at a window start, a close continuation: SPEC.md has the announcement check wait until this dialog
//! closes, and the layer manager tells no one when a layer closes. The continuation runs when the dialog's entity is
//! released, which the layer's closing does by whatever route (Cancel, the ×, Escape, the close after the thanks, a
//! reset), since the layer holds the dialog's only strong reference.
//!
//! NOTE: Not ported. The old app announced a successful send once more in a status region outside the dialog after it
//! closed, because its dialog's own region went inert with the modal; here the thank-you line is itself a status,
//! announced while it shows. The old app's failure for a helm it could not reach at all ("the request to Farhelm
//! failed") has no counterpart: a lost channel resets the window, which closes the dialog.

#[cfg(test)]
mod tests;

use std::rc::Rc;
use std::time::Duration;

use farhelm_helm_client::{ApiPath, Method, PathError};
use farhelm_proto::feedback::{
    FEEDBACK_OS_MAX_CHARS, FEEDBACK_VERSION_MAX_CHARS, FeedbackSubmission, FeedbackSurface,
    fit_machine_field, is_blank,
};
use farhelm_proto::ui_channel::{PREFERENCES_KEY, Preferences};
use gpui::{
    AnyWindowHandle, App, AppContext as _, ClickEvent, Context, FocusHandle, FontWeight,
    InteractiveElement as _, IntoElement, ParentElement as _, Render, SharedString,
    StatefulInteractiveElement as _, Styled as _, Subscription, WeakEntity, Window, div, px, svg,
};

use crate::assets::Icon;
use crate::components::{
    Button, Checkbox, DialogFrame, HoverHelp, Role, TextInput, TextInputEvent, TextInputState,
    Tier, capitalize_validation,
};
use crate::focus::{self, Mark, ReturnTo};
use crate::host_text::{HostSentence, HostText};
use crate::layers::{self, Dialog, DismissHook, Dismissal, LayerId, MainAction};
use crate::operations::{Command, OpKey, Outcome, Refusal, Work};
use crate::theme::{self, Palette, metrics};
use crate::{session, ticker};

// ---- Wording and sizes -----------------------------------------------------------------------------------------------

/// What a successful send says, in the dialog's body until it closes.
pub(crate) const THANKS: &str = "Thanks. Your feedback is on its way to Farhelm's maintainer.";

/// How long the thanks stays on screen before the dialog closes itself, the old `THANKS_MS`.
pub(crate) const THANKS_SHOWN: Duration = Duration::from_millis(1_500);

/// The start of every refusal the helm's feedback route words itself; anything else it answers with (axum's own text
/// for a body it cannot read, say) is not written for people and is replaced ([`failure_words`]).
const FAILURE_PREFIX: &str = "Couldn't send feedback:";

/// Said after a failure's reason, so the user knows nothing they typed is lost.
const KEPT: &str = " Your text is still here, so you can try again or copy it.";

/// The route, as a refusal without words names it.
const SHOWN: &str = "POST /api/feedback";

/// `.feedback-dialog`: `width: min(560px, 100%)`.
const WIDTH: f32 = 560.;
/// `.feedback-glyph`: the header tile's glyph.
const GLYPH: f32 = 28.;
/// `.feedback-dialog .host-settings-body`: the gap between the body's parts.
const BODY_GAP: f32 = 16.;
/// `.feedback-field`: the gap between a field's label and its input.
const FIELD_GAP: f32 = 6.;
/// `.feedback-dialog .feedback-summary`: the gap between the summary's two lines.
const SUMMARY_GAP: f32 = 4.;
/// Where the message box starts; it grows with its text from there. The old box was `rows="5"` at the font's normal
/// line height, about 97 pixels high in the reference screenshots; the growing box draws its rows at a line height of
/// 1.45, so four of its rows come to about that height, where five would be some 15 pixels taller.
const MESSAGE_ROWS: usize = 4;
/// The line heights of the old dialog's text at the browser's `line-height: normal` for Inter, which GPUI's default
/// (much taller) does not match: the 18-pixel title, and the 12-pixel summary lines and field labels.
const TITLE_LINE: f32 = 22.;
const SMALL_LINE: f32 = 15.;
/// The attached box's rows: 18 pixels apart in the reference screenshots, with the 2-pixel gap between them.
const ATTACHED_LINE: f32 = 16.;
/// `.feedback-attached`: its padding, vertical then horizontal.
const ATTACHED_PAD: (f32, f32) = (10., 12.);
/// `.feedback-attached dl`: the gaps between its rows and between a term and its value.
const ATTACHED_ROW_GAP: f32 = 2.;
const ATTACHED_COLUMN_GAP: f32 = 12.;
/// `.feedback-attached > .host-settings-help`: the room under the box's heading.
const ATTACHED_HEADING_GAP: f32 = 6.;
/// The terms' column: as wide as the longest term ("Operating system") at 12 pixels, the old grid's `max-content`. The
/// old browser drew that term about 99 pixels wide; GPUI's Inter comes out a few pixels wider, and at 100 the term
/// wrapped onto a second line, so the column has a little room to spare.
const ATTACHED_TERM_WIDTH: f32 = 104.;

// ---- Opening ---------------------------------------------------------------------------------------------------------

/// Open the dialog on top of everything else in `window`: the entry point for the Help menu's Send feedback.
///
/// `opener` is the control closing returns focus to ([`ReturnTo::Control`]), also where the click that opened it left
/// that control unfocused (macOS): from the Help menu, the `?` button, not the menu item, which goes with its popover.
/// `mark` is the [`Mark`] taken in the handler of the user's action; nothing opens when the user has moved on since or
/// the window is inert. The contact starts as the remembered one in the pushed preferences, as they are now, with
/// Re-use checked. Returns the dialog's layer, or `None` when nothing opened.
pub fn open(
    opener: &FocusHandle,
    mark: &Mark,
    window: &mut Window,
    cx: &mut App,
) -> Option<LayerId> {
    open_returning(ReturnTo::Control(opener.clone()), None, mark, window, cx)
}

/// What [`open_returning`] runs once the dialog has closed: told the window it closed in.
pub type OnClosed = Box<dyn FnOnce(&mut Window, &mut App)>;

/// [`open`] for the Welcome card's Send feedback now: closing returns focus as `returns_to` says (the card's own
/// target: the `?` button for a card reopened from Help, home for the one that opened by itself), and `on_closed`, if
/// given, runs once the dialog has closed.
///
/// `on_closed` runs exactly once, after the dialog's layer closed by any route, a reset's included, so a caller that
/// must not act for a retired UI session checks that itself. It runs outside every update but the window's, so it may
/// open a layer. It never runs when the dialog did not open (the user moved on since `mark`, or the window is inert):
/// then there is nothing to wait for, and the caller learns that from the `None` this returns.
pub fn open_returning(
    returns_to: ReturnTo,
    on_closed: Option<OnClosed>,
    mark: &Mark,
    window: &mut Window,
    cx: &mut App,
) -> Option<LayerId> {
    let store = session::current_store(cx)?;
    let remembered = store
        .read(cx)
        .collections()
        .get::<Preferences>(PREFERENCES_KEY)
        .and_then(|preferences| preferences.feedback_contact.clone())
        .unwrap_or_default();
    let dialog = cx.new(|cx| FeedbackDialog::new(&remembered, window, cx));
    let start = dialog.read(cx).message.focus_handle(cx);
    // Enter on the dialog itself (after a click on its empty space) sends, unless Send is unavailable.
    let weak = dialog.downgrade();
    let main_action: MainAction = Rc::new(move |window, cx| {
        let _ = weak.update(cx, |dialog, cx| dialog.send(window, cx));
    });
    // Escape and the header Close follow Cancel's rule: nothing closes the dialog while a send is out.
    let on_dismiss: DismissHook = Rc::new(move |_, cx| {
        if may_close(cx) {
            Dismissal::Close
        } else {
            Dismissal::Stay
        }
    });
    let layer = layers::open_dialog(
        Dialog {
            view: dialog.clone().into(),
            container: dialog.read(cx).container.clone(),
            // The message field, as the old dialog started there.
            start,
            // None: the dialog is about nothing the helm pushes, so only the user (or a reset) closes it.
            subject: None,
            main_action: Some(main_action),
            // SPEC.md, "Opening and closing", does not list Send feedback among the dialogs that keep focus on the
            // control in use when their empty space is clicked.
            keeps_focus_on_empty_click: false,
            returns_to,
            on_dismiss: Some(on_dismiss),
            placement: layers::DialogPlacement::Centered,
            dismiss_on_outside_press: false,
        },
        mark,
        window,
        cx,
    )?;
    dialog.update(cx, |dialog, cx| {
        dialog.layer = Some(layer);
        // Registered only once the dialog is open, so a refused open (the entity dropped at once) runs nothing. The
        // layer holds the dialog's one strong reference (the send's completion, the thanks' timer and the handlers all
        // hold it weakly), so the release comes with the layer's closing.
        dialog._on_closed = on_closed
            .map(|on_closed| cx.on_release_in(window, move |_, window, cx| on_closed(window, cx)));
    });
    #[cfg(test)]
    layers::record_opened(&dialog, cx);
    Some(layer)
}

// ---- The rules, as plain functions -----------------------------------------------------------------------------------

/// What the dialog's last send came to. Whether a send is out right now is not here: that is the operation store's,
/// read through [`sending`], so the dialog keeps no copy of it that could disagree with the store.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Phase {
    /// Nothing sent yet (or a send is out for the first time): the user is writing.
    Editing,
    /// The helm forwarded it; the dialog thanks the user and closes itself shortly.
    Sent,
    /// The last send failed, for the reason shown ([`failure_words`]); the text is kept for a retry.
    Failed(HostText),
}

/// Whether a feedback send is out: work in flight on the feedback key in the current UI session's operation store.
/// Read wherever the dialog asks, while drawing too, so the busy Send, the disabled closes and the dismissal hook all
/// follow the store, and a view that read it while drawing redraws when the store settles the send.
fn sending(cx: &App) -> bool {
    session::current_operations(cx)
        .is_some_and(|operations| operations.read(cx).in_flight(&OpKey::feedback()).is_some())
}

/// Whether the dialog may close now: not while a send is out ([`sending`]). Closing then would drop the send's outcome
/// with no word on whether it arrived, and lose the typed text, which SPEC.md promises a failed send keeps. Every user
/// close path asks this when the close runs (Escape and the header Close through the dismissal hook, Cancel through the
/// same dismissal), not as the dialog was drawn, so a close queued behind Send before the redraw that disables Cancel
/// still waits. The old app's `may_close`.
fn may_close(cx: &App) -> bool {
    !sending(cx)
}

/// The contact to remember after a successful send: exactly the contact `sent` when Re-use was checked (`reuse`), and
/// none (the remembered contact cleared) when it was unchecked or nothing was sent as the contact. Apply it only after
/// success; editing, Cancel and a failure leave the remembered contact as it is.
fn contact_after_success(sent: Option<String>, reuse: bool) -> Option<String> {
    sent.filter(|contact| reuse && !is_blank(contact))
}

/// The operating system's name as the dialog shows and sends it: "macOS" or "Linux", the platforms the app ships on,
/// read at build time. Anything else gives an empty name, which [`fit_machine_field`] shows as `unknown`. Nothing more
/// specific (a version, a machine model) is read, on purpose.
fn os_name() -> &'static str {
    if cfg!(target_os = "macos") {
        "macOS"
    } else if cfg!(target_os = "linux") {
        "Linux"
    } else {
        ""
    }
}

/// The surface this client sends: the desktop app, the only client there is.
const SURFACE: FeedbackSurface = FeedbackSurface::Desktop;

/// How the dialog names [`SURFACE`] to the user, apart from its wire word (the old app's `surface_label`).
const SURFACE_LABEL: &str = "Desktop app";

/// What a refused send says: the helm's own sentence when its feedback route wrote one (they all start with
/// [`FAILURE_PREFIX`], and may quote nothing the user cannot see, but travel as host text like every refusal), and
/// otherwise a plain sentence naming the status, since anything else the helm answers with (axum's "length limit
/// exceeded" for an oversized body, a route's bare status) is not written for people. Ported from the old app's
/// `api::send_feedback`.
fn failure_words(refusal: &Refusal) -> HostText {
    if refusal.words.raw().trim_start().starts_with(FAILURE_PREFIX) {
        refusal.words.clone()
    } else {
        HostText::from_host(format!(
            "{FAILURE_PREFIX} Farhelm could not take the request (HTTP {}).",
            refusal.status
        ))
    }
}

/// `POST /api/feedback` with `submission` as its body, as work on the feedback key.
fn command(submission: &FeedbackSubmission) -> Result<Command, PathError> {
    // A struct of strings and an enum always serializes.
    let body = serde_json::to_value(submission).unwrap_or_default();
    Ok(Command::new(
        Work::SendFeedback,
        Method::Post,
        ApiPath::api(&["feedback"])?,
        Some(body),
        SHOWN,
    ))
}

/// Remember `contact` with the helm, or clear the remembered one (`None`): a keyless, best-effort write
/// ([`crate::operations::send`]), whose failure is logged and never shown, since the feedback itself was sent. The log
/// line names no contact: SPEC_impl.md keeps the contact out of every refusal and log.
fn remember(contact: Option<String>, cx: &mut App) {
    let path = match ApiPath::api(&["preferences"]) {
        Ok(path) => path,
        Err(error) => {
            tracing::warn!(%error, "could not remember the feedback contact");
            return;
        }
    };
    crate::operations::send(
        Method::Put,
        path,
        Some(serde_json::json!({ "feedback_contact": contact })),
        "PUT /api/preferences",
        cx,
        |answer, _| {
            if answer.is_err() {
                tracing::warn!("could not remember the feedback contact");
            }
        },
    );
}

// ---- The dialog ------------------------------------------------------------------------------------------------------

/// The open dialog: its two fields, the Re-use choice, the fitted values sent with them, what the last send came to,
/// and what closes it.
pub(crate) struct FeedbackDialog {
    /// The message: a growing box, [`MESSAGE_ROWS`] rows to start, which scrolls once it reaches the growing box's cap.
    message: TextInputState,
    /// How to reach the user: one line, starting as the remembered contact.
    contact: TextInputState,
    /// Re-use for future feedback: checked when the dialog opens, shown only while the contact is not blank.
    reuse: bool,
    /// The version the sidebar shows, fitted: what is displayed and sent.
    version: String,
    /// The operating system's name, fitted: what is displayed and sent.
    os: String,
    phase: Phase,
    /// The footer's Cancel, which reads Close once the send succeeded: where focus goes then.
    cancel: FocusHandle,
    /// The dialog's layer container. Focus rests here, on no control, after a press no control takes (a click on Send
    /// under macOS's rule, a click on empty space), which is how [`settle`] tells that a refusal may hand focus back to
    /// the message.
    container: FocusHandle,
    /// Set right after the layer opens.
    layer: Option<LayerId>,
    /// The thanks' timer, once the send succeeded. Dropped with the dialog, which cancels it.
    closing: Option<ticker::Deadline>,
    /// Each field's edits, which redraw the dialog (Send's availability, Re-use's presence, the size line).
    _edits: [Subscription; 2],
    /// Runs [`open_returning`]'s close continuation when the dialog is released; `None` without one.
    _on_closed: Option<Subscription>,
}

impl FeedbackDialog {
    /// A dialog with an empty message and `remembered` as the contact.
    fn new(remembered: &str, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let message = TextInputState::new_growing(MESSAGE_ROWS, window, cx);
        let contact = TextInputState::new(window, cx);
        contact.set_text(remembered.to_owned(), window, cx);
        // Enter in the contact line submits nothing: the old field sat in no form, so Enter there did nothing either.
        let redraw = |_: &mut Self, event, _: &mut Window, cx: &mut Context<Self>| {
            if event == TextInputEvent::Changed {
                cx.notify();
            }
        };
        let edits = [
            message.subscribe(window, cx, redraw),
            contact.subscribe(window, cx, redraw),
        ];
        FeedbackDialog {
            message,
            contact,
            reuse: true,
            version: fit_machine_field(farhelm_proto::BUILD_VERSION, FEEDBACK_VERSION_MAX_CHARS),
            os: fit_machine_field(os_name(), FEEDBACK_OS_MAX_CHARS),
            phase: Phase::Editing,
            cancel: cx.focus_handle().tab_stop(true),
            container: cx.focus_handle(),
            layer: None,
            closing: None,
            _edits: edits,
            _on_closed: None,
        }
    }

    /// The submission as the dialog shows it now: exactly what Send would send.
    fn submission(&self, cx: &App) -> FeedbackSubmission {
        let contact = self.contact.text(cx).to_string();
        FeedbackSubmission {
            message: self.message.text(cx).to_string(),
            contact: (!is_blank(&contact)).then_some(contact),
            version: self.version.clone(),
            surface: SURFACE,
            os: self.os.clone(),
        }
    }

    /// Send the submission, unless it is invalid, a send is already out, or one already succeeded. Called by Send, and
    /// by Enter on the dialog itself; both keep their own guard here, since a second press can arrive before the
    /// redraw that draws Send busy, and a second send would file a second issue. (The operation store would refuse a
    /// second start on the feedback key too; the guard keeps that refusal from being the only line of defence.)
    fn send(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if sending(cx) || self.phase == Phase::Sent {
            return;
        }
        let outgoing = self.submission(cx);
        if outgoing.validate().is_err() {
            return;
        }
        let Some(operations) = session::current_operations(cx) else {
            return;
        };
        let command = match command(&outgoing) {
            Ok(command) => command,
            Err(error) => {
                tracing::warn!(%error, "feedback not sent");
                return;
            }
        };
        let reuse = self.reuse;
        // Taken in the press's handler, so the focus move after a success can tell whether the user moved on since.
        let mark = Mark::take(cx);
        let dialog = cx.weak_entity();
        let handle = window.window_handle();
        let started = operations.update(cx, |operations, cx| {
            operations.start(&OpKey::feedback(), command, cx, move |settled, cx| {
                settle(
                    &dialog,
                    settled.outcome.clone(),
                    outgoing,
                    reuse,
                    mark,
                    handle,
                    cx,
                );
            })
        });
        match started {
            Ok(()) => {
                // Read-only rather than disabled, so a field that holds focus keeps it, as the busy Send does.
                self.message.set_read_only(true, cx);
                self.contact.set_read_only(true, cx);
                cx.notify();
            }
            // Busy cannot happen (the guard above returned while a send is out); a retired store means the window is
            // inert.
            Err(not_started) => tracing::debug!(?not_started, "feedback not sent"),
        }
    }

    /// The send settled with `outcome`: record what the dialog now shows.
    fn settled(&mut self, outcome: &Outcome, cx: &mut Context<Self>) {
        self.message.set_read_only(false, cx);
        self.contact.set_read_only(false, cx);
        self.phase = match outcome {
            Outcome::Refused(refusal) => Phase::Failed(failure_words(refusal)),
            // The route answers 204; any success counts, since nothing here waits for long work.
            Outcome::Done(_) | Outcome::Accepted(_) => Phase::Sent,
        };
        cx.notify();
    }

    /// The local problem to show under the fields: a message or contact the helm would refuse for its size, said only
    /// once the message is not blank, since a blank one just leaves Send disabled.
    fn problem(&self, cx: &App) -> Option<String> {
        let submission = self.submission(cx);
        if is_blank(&submission.message) {
            return None;
        }
        submission.validate().err().map(capitalize_validation)
    }
}

/// The completion of the send `dialog` started with `outgoing`: show the outcome in the dialog if it is still open,
/// and after a success remember or clear the contact, move focus to the footer's Close and start the thanks' timer.
/// After a refusal, focus on the dialog itself goes to the message (the old app's `refocus`), unless the user moved on
/// since `mark`; focus on any control (Send, or a field the user clicked meanwhile) stays where it is. It runs outside
/// every entity's update, so it may update the window.
fn settle(
    dialog: &WeakEntity<FeedbackDialog>,
    outcome: Outcome,
    outgoing: FeedbackSubmission,
    reuse: bool,
    mark: Mark,
    handle: AnyWindowHandle,
    cx: &mut App,
) {
    let sent = matches!(outcome, Outcome::Done(_) | Outcome::Accepted(_));
    if sent {
        // Whether or not the dialog is still there: the send succeeded, so the choice the user made with it stands.
        remember(contact_after_success(outgoing.contact, reuse), cx);
    }
    let Ok((target, container, layer)) = dialog.update(cx, |dialog, cx| {
        dialog.settled(&outcome, cx);
        let target = if sent {
            dialog.cancel.clone()
        } else {
            dialog.message.focus_handle(cx)
        };
        (target, dialog.container.clone(), dialog.layer)
    }) else {
        return;
    };
    // After a success the fields, and Send, are gone from the body; the old dialog put focus on its Close for the same
    // reason.
    let _ = handle.update(cx, |_, window, cx| {
        if sent || container.is_focused(window) {
            focus::focus_in_layer(&target, &mark, window, cx);
        }
    });
    if !sent {
        return;
    }
    let Some(layer) = layer else {
        return;
    };
    let closing = ticker::after(THANKS_SHOWN, cx, move |cx| {
        let _ = handle.update(cx, |_, window, cx| layers::close(layer, window, cx));
    });
    let _ = dialog.update(cx, |dialog, _| dialog.closing = Some(closing));
}

/// A click handler that dismisses the dialog through the layer manager, as Escape does, so the dismissal hook decides:
/// Cancel and the header Close. Called from the click, not from inside an update of the dialog, because the hook reads
/// the dialog.
fn dismiss(dialog: WeakEntity<FeedbackDialog>) -> impl Fn(&ClickEvent, &mut Window, &mut App) {
    move |_, window, cx| {
        let layer = dialog.upgrade().and_then(|dialog| dialog.read(cx).layer);
        if let Some(layer) = layer {
            layers::dismiss(layer, window, cx);
        }
    }
}

// ---- Drawing ---------------------------------------------------------------------------------------------------------

/// One labelled field (`.feedback-field`): its words over its input, 6 pixels apart, the words at 12 pixels in the
/// quieter ink. The words are drawn text, not a node; the input carries the same words as its accessible name. Tests
/// find the input by `selector`, which covers the input alone, so a click at its center lands in it.
fn field(
    selector: &'static str,
    label: &'static str,
    input: TextInput,
    palette: &Palette,
) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap(px(FIELD_GAP))
        .child(
            div()
                .text_size(metrics::TEXT_SM)
                .line_height(px(SMALL_LINE))
                .text_color(theme::color(palette.fg_1))
                .child(label),
        )
        .child(
            div()
                .debug_selector(move || selector.to_owned())
                .child(input),
        )
}

/// A line of the app's words in the danger ink (`.feedback-error`), announced as an alert and found by `id`.
fn problem_line(id: &'static str, words: String, palette: &Palette) -> impl IntoElement {
    div()
        .id(id)
        .debug_selector(move || id.to_owned())
        .role(Role::Alert)
        .aria_label(SharedString::from(words.clone()))
        .text_size(metrics::TEXT_SM)
        .text_color(theme::color(palette.danger))
        .child(words)
}

/// The header's title and its two summary lines (`.feedback-summary`): where the feedback goes, and the public
/// alternative with its Chat on Discord link, which opens the invite in the browser and leaves the dialog and its text
/// as they are. Drawn in the header ahead of Close, so the link comes before Close in Tab order, as it did.
fn heading(palette: &Palette) -> impl IntoElement {
    let discord = Button::new(
        "feedback-discord",
        "Chat on Discord ↗",
        HoverHelp::tooltip(
            "Chat on Discord: talk with Farhelm's maintainer and other users in the open",
        ),
        Role::Link,
    )
    .accessible_name("Chat on Discord")
    .tier(Tier::Link)
    .on_click(|_, _, cx| cx.open_url(crate::help_menu::DISCORD_URL));
    div()
        .flex()
        .flex_col()
        .child(
            div()
                .text_size(metrics::TEXT_TITLE)
                .line_height(px(TITLE_LINE))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme::color(palette.fg_0))
                .child("Send feedback"),
        )
        .child(
            div()
                .mt(px(6.))
                .flex()
                .flex_col()
                .items_start()
                .gap(px(SUMMARY_GAP))
                .text_size(metrics::TEXT_SM)
                .line_height(px(SMALL_LINE))
                .text_color(theme::color(palette.fg_1))
                .child("Goes privately to Farhelm's maintainer, not a public tracker")
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .flex_wrap()
                        .items_center()
                        .child("Rather talk in the open?\u{a0}")
                        .child(discord),
                ),
        )
}

/// The box of what is sent besides the fields (`.feedback-attached`): a heading line, then the version (in the
/// monospace face, as the old `.feedback-version`), the surface and the operating system, each beside its term.
fn attached(version: &str, os: &str, palette: &Palette) -> impl IntoElement {
    let row = |term: &'static str, id: &'static str, value: String, mono: bool| {
        let value = div()
            .id(id)
            .debug_selector(move || id.to_owned())
            .role(Role::Label)
            .aria_label(SharedString::from(value.clone()))
            .flex_1()
            .min_w_0()
            .text_color(theme::color(palette.fg_0))
            .child(value);
        div()
            .flex()
            .flex_row()
            .gap(px(ATTACHED_COLUMN_GAP))
            .child(
                div()
                    .flex_none()
                    .w(px(ATTACHED_TERM_WIDTH))
                    .whitespace_nowrap()
                    .text_color(theme::color(palette.fg_1))
                    .child(term),
            )
            .child(if mono {
                value.font_family(crate::assets::TERMINAL_FONT)
            } else {
                value
            })
    };
    div()
        .flex()
        .flex_col()
        .py(px(ATTACHED_PAD.0))
        .px(px(ATTACHED_PAD.1))
        .border_1()
        .border_color(theme::color(palette.border_dim))
        .rounded(metrics::RADIUS_SM)
        .bg(theme::color(palette.bg_1))
        .text_size(metrics::TEXT_SM)
        .child(
            div()
                .mb(px(ATTACHED_HEADING_GAP))
                .line_height(px(SMALL_LINE))
                .text_color(theme::color(palette.fg_1))
                .child("Sent with it, and nothing else"),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(ATTACHED_ROW_GAP))
                .line_height(px(ATTACHED_LINE))
                .child(row("Version", "feedback-version", version.to_owned(), true))
                .child(row(
                    "App",
                    "feedback-surface",
                    SURFACE_LABEL.to_owned(),
                    false,
                ))
                .child(row("Operating system", "feedback-os", os.to_owned(), false)),
        )
}

impl Render for FeedbackDialog {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = theme::palette(cx);
        let weak = cx.weak_entity();
        let sending = sending(cx);
        let sent = self.phase == Phase::Sent;
        let body = div().flex().flex_col().gap(px(BODY_GAP));
        let body = if sent {
            body.child(
                div()
                    .id("feedback-thanks")
                    .debug_selector(|| "feedback-thanks".into())
                    .role(Role::Status)
                    .aria_label(THANKS)
                    .text_size(metrics::TEXT_MD)
                    .text_color(theme::color(palette.fg_0))
                    .child(THANKS),
            )
        } else {
            let contact_shown = !is_blank(&self.contact.text(cx));
            let toggle = weak.clone();
            let reuse = contact_shown.then(|| {
                // `.feedback-dialog .app-settings-choice`: the choice's words at 13 pixels.
                div().text_size(metrics::TEXT_MD).child(
                    Checkbox::new(
                        "feedback-reuse",
                        "Re-use for future feedback",
                        HoverHelp::tooltip(
                            "Reuse: remember this contact on the helm after a successful send",
                        ),
                        Role::CheckBox,
                    )
                    .checked(self.reuse)
                    .disabled(sending)
                    .on_toggle(move |checked, _, cx| {
                        let _ = toggle.update(cx, |dialog, cx| {
                            dialog.reuse = checked;
                            cx.notify();
                        });
                    }),
                )
            });
            // The reason is host text (the helm's words), so it is a part of its own in a host sentence; the alert is
            // named by the sentence's escaped text.
            // A retry's send replaces the last failure's line while it is out, as the old dialog's did.
            let failure = match &self.phase {
                Phase::Failed(reason) if !sending => {
                    let sentence = HostSentence::new("feedback-failure-words")
                        .host(reason)
                        .text(KEPT);
                    Some(
                        div()
                            .id("feedback-failure")
                            .debug_selector(|| "feedback-failure".into())
                            .role(Role::Alert)
                            .aria_label(sentence.accessible_name())
                            .text_size(metrics::TEXT_SM)
                            .text_color(theme::color(palette.danger))
                            .child(sentence),
                    )
                }
                _ => None,
            };
            body.child(field(
                "feedback-message-field",
                "Message",
                TextInput::new(
                    "feedback-message",
                    &self.message,
                    "Message",
                    HoverHelp::tooltip("Your message to Farhelm's maintainer"),
                ),
                palette,
            ))
            .child(field(
                "feedback-contact-field",
                "How to reach you (optional)",
                TextInput::new(
                    "feedback-contact",
                    &self.contact,
                    "How to reach you (optional)",
                    HoverHelp::tooltip(
                        "How to reach you, if you want an answer: an email address or any other contact",
                    ),
                ),
                palette,
            ))
            .children(reuse)
            .child(attached(&self.version, &self.os, palette))
            .children(
                self.problem(cx)
                    .map(|problem| problem_line("feedback-problem", problem, palette)),
            )
            .children(failure)
        };
        let can_send = !sending && !sent && self.submission(cx).validate().is_ok();
        let cancel = Button::new(
            "feedback-cancel",
            if sent { "Close" } else { "Cancel" },
            HoverHelp::tooltip(if sent {
                "Close: your message was sent"
            } else {
                "Cancel: close without sending"
            }),
            Role::Button,
        )
        .tier(Tier::Neutral)
        .disabled(sending)
        .focus(&self.cancel)
        .on_click(dismiss(weak.clone()));
        let mut answers = vec![cancel];
        if !sent {
            let send = weak.clone();
            answers.push(
                Button::new(
                    "feedback-send",
                    if sending { "Sending…" } else { "Send" },
                    HoverHelp::tooltip("Send: deliver this message privately to the maintainer"),
                    Role::Button,
                )
                .tier(Tier::Primary)
                // Busy, not disabled, while the send is out: a disabled button gives up focus, and the press that sent
                // should leave focus where the user put it until the outcome is known.
                .busy(sending)
                .disabled(!sending && !can_send)
                .on_click(move |_, window, cx| {
                    let _ = send.update(cx, |dialog, cx| dialog.send(window, cx));
                }),
            );
        }
        let frame = DialogFrame::new("feedback", "Send feedback", body, answers)
            .heading(heading(palette))
            .tile(
                svg()
                    .path(Icon::Feedback.path())
                    .size(px(GLYPH))
                    .text_color(theme::color(palette.fg_0)),
            )
            .width(px(WIDTH))
            // The old body's stop, named for what it held: after a successful send the body is only the thank-you
            // line, and a short window must still be able to scroll it from the keyboard.
            .scroll_stop(if self.phase == Phase::Sent {
                "Feedback sent"
            } else {
                "Feedback"
            })
            .close(
                "Close send feedback",
                HoverHelp::tooltip("Close: leave this feedback dialog"),
                sending,
                dismiss(weak),
            );
        // No note while a send is out: whether anything was sent is what the user is waiting to learn.
        match &self.phase {
            _ if sending => frame,
            Phase::Editing => frame.note("Nothing is sent until you press Send"),
            Phase::Failed(_) => frame.note("Nothing was sent"),
            Phase::Sent => frame,
        }
    }
}
