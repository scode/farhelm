//! The session launcher's dialog: what the user sees of a [`LaunchDraft`] and how their input reaches it.
//!
//! SPEC.md, "Creation" (the launcher layout paragraph and the launch composer bullet) and "Keyboard focus". Ported from
//! the old app's `list/create_form.rs` and `launch_controls.rs`, and the `.create-session-form` and `.launch-composer-*`
//! rules in its stylesheet, at reference commit `24bc0580`. Here's the TLDR:
//!
//! - **The frame.** The shared dialog frame, 960 pixels wide. The command field and Close keep their own line in the
//!   header above its guidance and bounded notices. The pinned footer holds Cancel, Reset choices (Agent tab),
//!   Save as template (where offered) and the main action. Confirmations and refusals are bounded beneath them.
//!   Save as template opens its inline panel
//!   at the end of the body (`crate::templates::save_panel`), which holds the launcher's Launch while it is open.
//! - **The body**, in reading and Tab order: the recent setups while any match
//!   ([`recents`]), the Destination card (Folder or Managed checkout, then the Host, Folder and Browse address and
//!   Name). Host also offers new environments, which put their placement controls beside Host in place of the
//!   editable folder ([`placement`]). A managed checkout puts Repository and the preview there. Next come the
//!   Agent and Command tabs, and the chosen tab's fields. The other tab's fields are not drawn, but its draft is kept
//!   (SPEC.md: "switching tabs keeps each tab's draft"). Where the Agent tab offers model, effort and permissions,
//!   [`choices`] keeps them together and wraps entire fields to fit the available width.
//!   [`more_options`] keeps the less common Agent settings reachable and opens their editors when the draft needs them.
//! - **Input to edits.** Every field's change becomes one of the draft's edit methods; nothing here decides a launch
//!   rule. A field the draft changes on its own (Reset choices clearing the wrapper prefix) is put back in step after
//!   the edit.
//! - **The main action.** Launch (Replace, Restart for those modes) is drawn retired while `LaunchDraft::launch_refusal`
//!   names a reason, and the reason is its hover text; a press, or Enter on the dialog, a single-line field or a
//!   choice, shows the reason beside it in the bordered alert, "Not launched." first ("Restart error." in Restart
//!   with). A press of a complete New, Clone, Replace with or Restart with launches it ([`super::launch`]): the
//!   dialog closes, and the same launcher comes back with the helm's words in that alert if the helm refuses. A new
//!   sandbox asks "Create a sandbox for this session?" before it sends anything ([`sandbox_question`]). Replace
//!   with also says beside it what replacing stops while the source has anything alive, and Restart with that it stops
//!   a working agent first, its action then reading "Stop and restart"; Restart with marks each edited setting
//!   ([`changed`]). Its accessible name keeps the
//!   operation and its target, as the old one did.
//! - **Reads.** The model catalog is read when the launcher opens (`GET /api/launch-catalog`), including a quick
//!   launch's prepared out of sight ([`prepare_unseen`]), and again on Retry after a failed read. A template batch that
//!   arrives while the read is out waits for it ([`Launcher::apply_batch`]). The Tensorlake account is read once beside
//!   it (`GET /api/tensorlake`, `placement`), for New sandbox's menu row and prerequisites, and a quick launch waits for
//!   both reads before it applies its templates. Hosts, sessions and the remembered permission are read from the store
//!   while drawing, and the remembered wrapper prefixes when an agent type is chosen.
//!
//! NOTE: Not built yet, each with the piece that brings it: the rest of managed checkouts (`checkout` says which); and
//! keyboard scrolling of an overflowing notice (the old region took focus once it overflowed; today's refusals are one
//! or two lines). Launch text holding invisible characters is spelled out by `shown_text`. The model field is
//! `model_field`, the host picker
//! `host_picker`, Restart with's "● Changed" markers `changed`, the managed-checkout destination `checkout`, Browse
//! `browse`, recent setup edits `recents`, the escalation banner `escalation`, Save as template's side `save`. The
//! floating lists under the model, folder, search and repository fields share `field_list`, and the debounced reads
//! (the checkout's and the folder check's) share `latest`.

use std::rc::Rc;
use std::time::Duration;

use farhelm_proto::custom_agents::CustomAgent;
use farhelm_proto::launcher::LaunchCatalogModel;
use farhelm_proto::recipes::RecipeSize;
use farhelm_proto::ui_channel::{
    HostStateView, HostView, PREFERENCES_KEY, Preferences, SessionRow,
};
use farhelm_proto::{DeleteGuard, LaunchHarness, LaunchPermission, SessionLaunch, SessionStatus};
use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, AnyWindowHandle, App, AppContext as _, ClickEvent, Context, Div, ElementId, Entity,
    FontWeight, Global, InteractiveElement as _, IntoElement, ParentElement as _, Render,
    SharedString, Stateful, StatefulInteractiveElement as _, Styled as _, Subscription, WeakEntity,
    Window, div, px, svg,
};

use super::draft::{
    Catalog, FOLDER_INSTALLATION_MOVED, Field, FolderDefault, Install, LaunchDraft, Mode, Tab,
    harness_label, remembered_permission,
};
use super::launch::{self, Request};
use super::read_model_catalog;
use super::request::{Account, Recipes, Refusal, SELECT_REPOSITORY, Target};
use super::words::agent_note;
use crate::components::{
    Button, Checkbox, DialogFrame, HoverHelp, RadioGroup, RadioOption, Role, Select, SelectOption,
    TabOption, TabStrip, TextInput, TextInputEvent, TextInputState, Tier, return_key_content,
    scroll_from_controls, scroll_handle,
};
use crate::focus::{Mark, ReturnTo};
use crate::host_text::{self, HostSentence, HostText};
use crate::hosts::identity::{display_name, phase};
use crate::layers::{self, Dialog, DialogPlacement, LayerId, MainAction};
use crate::operations::OpKey;
use crate::prompt::{ActionFrame, ShownAction};
use crate::session_list::replace;
use crate::store::HelmStore;
use crate::templates::checkout_name;
use crate::templates::save_panel::SavePanel;
use crate::theme::{self, metrics};
use crate::{session, ticker};
use changed::Changed;

mod browse;
mod changed;
mod checkout;
#[cfg(test)]
pub(super) use checkout::repository_choices as repository_choices_for_tests;
mod choices;
mod escalation;
mod field_list;
#[cfg(test)]
pub(super) use field_list::step as step_for_tests;
mod folder_check;
mod folder_list;
#[cfg(test)]
pub(super) use folder_list::{
    recent_folders as recent_folders_for_tests, tail_label as tail_label_for_tests,
};
mod history;
mod hook_question;
mod host_picker;
mod latest;
mod model_field;
mod more_options;
mod placement;
mod recents;
mod sandbox_question;
mod save;
mod search;
mod shown_text;
pub(in crate::launcher) mod yolo_question;

/// `.launch-composer-backdrop .create-session-form`: `width: min(960px, 100%)`.
const WIDTH: f32 = 960.;
/// The gap between form fields. Each label sits close to the field it describes.
const ROW_GAP: f32 = 12.;
const LABEL_GAP: f32 = 4.;
/// Match the Model row height. The Host and Browse segments sit inside the address's one-pixel border.
const ADDRESS_HEIGHT: gpui::Pixels = choices::MODEL_ROW_CONTROL_HEIGHT;
/// Reserve room for a host mark and its name while leaving most of the address for a path.
const ADDRESS_HOST_WIDTH: f32 = 188.;
/// Leave enough path space for a short folder alongside the host and Browse before Name wraps beneath them.
const ADDRESS_MIN_WIDTH: f32 = 348.;
/// The Name column can drop beneath the address as the available form width contracts.
const DESTINATION_NAME_WIDTH: f32 = 170.;
/// The Container host menu matches its control's width. Reserve room for labels and unavailable reasons before
/// letting the held home and Name share its row.
const CONTAINER_HOST_MIN_WIDTH: f32 = 370.;
/// `.launch-composer-columns`: the destination block and the choices are 20 pixels apart.
const SECTION_GAP: f32 = 20.;
/// The Command tab's agent type select's value for no declared agent; every other value is an agent's label, which are
/// unique.
const NO_AGENT: &str = "none";
/// The initial prompt starts three rows high, as the old `<textarea rows=3>`.
const PROMPT_ROWS: usize = 3;
/// The line under a prompt shown read-only because it holds an invisible character (`shown_text`), the old launcher's.
const PROMPT_SPELLED_OUT: &str =
    "Invisible formatting is shown escaped. Clear the message to replace it.";

/// Why Launch waits in Replace with or Restart with while the source has another request in flight (see
/// [`Launcher::source_busy`]): the old app disabled Launch without a reason while its page-wide operation token was
/// held, and this sentence is worded as the host change's (`launch::HOST_CHANGE_IN_FLIGHT`), since it is the same kind
/// of wait.
const SOURCE_BUSY: &str = "Wait for this session's other operation to finish";

/// Why Launch waits while the Save as template panel is open, the old launcher's words: the panel saves a template, and
/// a launch beside it would start a session the user had not finished choosing to keep.
const TEMPLATE_PANEL_OPEN: &str = "Finish or cancel saving the template before launching";

/// Open the launcher on `draft`, on top of everything in `window`. `mark` is the [`Mark`] taken in the handler of the
/// user's action that asked for it; nothing opens if the user moved on since (the layer manager's rule) or no UI
/// session is connected. Returns the launcher's layer.
///
/// Focus starts in the command field, or on Cancel for Restart with. Closing returns focus to the opener.
pub fn open(draft: LaunchDraft, mark: &Mark, window: &mut Window, cx: &mut App) -> Option<LayerId> {
    open_noted(draft, None, None, mark, window, cx)
}

/// [`open`], with `host_note` said under the Host row until the user picks a host: why an opening did not keep the host
/// its source names (Clone's, as the old launcher's `.create-session-host-note` said it); and `note`, if any, said
/// from the first frame until the draft is next edited or Launch is pressed: a quick launch's banner, in the notices
/// region, or its refusal below the command field ([`Note`]; `super::entry::open_with`).
pub(in crate::launcher) fn open_noted(
    draft: LaunchDraft,
    host_note: Option<&'static str>,
    note: Option<Note>,
    mark: &Mark,
    window: &mut Window,
    cx: &mut App,
) -> Option<LayerId> {
    let store = session::current_store(cx)?;
    let launcher = cx.new(|cx| {
        let mut launcher = Launcher::new(draft, &store, window, cx);
        launcher.host_picker.note = host_note;
        launcher.note = note;
        launcher
    });
    let layer = show(&launcher, mark, window, cx)?;
    read_catalog(launcher.downgrade(), cx);
    placement::check_container_hosts(cx);
    placement::read_account(launcher.downgrade(), cx);
    #[cfg(test)]
    layers::record_opened(&launcher, cx);
    Some(layer)
}

/// Draw `launcher` as the launcher's dialog, on top of everything in `window`: a new launcher from [`open`], or one a
/// submit hid, brought back with its draft (`launch`), or a quick launch's never drawn before ([`prepare_unseen`]),
/// which stops being one never shown here. Refused, returning `None`, as [`open`] is: if the user moved on since
/// `mark`, or with no UI session. The layer holds the launcher from then on, and closing it drops the launcher unless
/// something else holds it too.
pub(super) fn show(
    launcher: &Entity<Launcher>,
    mark: &Mark,
    window: &mut Window,
    cx: &mut App,
) -> Option<LayerId> {
    session::current_store(cx)?;
    let start = launcher.read(cx).start_focus(cx);
    let container = cx.focus_handle();
    let weak = launcher.downgrade();
    // Enter on the dialog itself (after a click on its empty space) presses the main action.
    let dismissing = weak.clone();
    let main_action: MainAction = Rc::new(move |window, cx| {
        let _ = weak.update(cx, |launcher, cx| launcher.press(window, cx));
    });
    let layer = layers::open_dialog(
        Dialog {
            view: launcher.clone().into(),
            container: container.clone(),
            start,
            subject: None,
            main_action: Some(main_action),
            // SPEC.md, "Opening and closing": "A primary click on empty space keeps focus on the control in use in the
            // session launcher, Restart with".
            keeps_focus_on_empty_click: true,
            returns_to: ReturnTo::Opener,
            // Close the command results first, as the narrow popover does, so Escape preserves the template draft.
            // While a setting or template write is out, the answer keeps the launcher open.
            on_dismiss: Some(Rc::new(move |window, cx| {
                let Some(launcher) = dismissing.upgrade() else {
                    return layers::Dismissal::Close;
                };
                if launcher.read(cx).yolo_in_flight(cx)
                    || launcher.read(cx).hook_write_in_flight(cx)
                    || launcher.read(cx).template_saving(cx)
                {
                    return layers::Dismissal::Stay;
                }
                if launcher.update(cx, |launcher, cx| launcher.dismiss_command_pane(window, cx)) {
                    return layers::Dismissal::Stay;
                }
                if launcher.read(cx).save_panel.is_some() {
                    launcher.update(cx, |launcher, cx| launcher.close_save_panel(window, cx));
                    return layers::Dismissal::Stay;
                }
                layers::Dismissal::Close
            })),
            placement: DialogPlacement::Centered,
            dismiss_on_outside_press: false,
        },
        mark,
        window,
        cx,
    )?;
    launcher.update(cx, |launcher, cx| {
        launcher.layer = Some(layer);
        // A launch the helm refused for YOLO comes back with its question, asked now that it can open in place.
        launcher.ask_pending_yolo(mark, window, cx);
        // So does a launch that waits for "Create a sandbox for this session?", a quick launch's out of sight included.
        launcher.ask_pending_sandbox(mark, window, cx);
        launcher.quick_launch_unseen = false;
    });
    cx.set_global(Shown {
        launcher: launcher.downgrade(),
        container,
    });
    Some(layer)
}

/// The launcher drawn last ([`show`]), weakly, with its dialog's container: what `super::entry::open_launcher` answers
/// from, checking the container against the layer stack. One app-wide slot, which assumes one launcher window, as the
/// desktop app has: a launcher shown in another window would take it. It is never cleared by hand, so after a close it
/// keeps a dead focus handle, which no layer stack holds and so never matches.
pub(super) struct Shown {
    pub(super) launcher: WeakEntity<Launcher>,
    pub(super) container: gpui::FocusHandle,
}

impl Global for Shown {}

/// What the launcher says for a quick launch that did not start, or a template that changed nothing, until the draft is
/// next edited: a banner in the notices region or a refusal below the command field. A host sentence keeps any
/// template name escaped.
#[derive(Clone, Debug, PartialEq)]
pub enum Note {
    /// A prominent banner: a quick launch's templates need more choices before they can launch (SPEC.md, "Quick
    /// launch" and "Quick-launch sequences"). The old `.partial-template-banner` look. It also goes on its own
    /// [`BANNER_SHOWS_FOR`] after the templates applied, as the old banner did whatever its origin.
    Banner(HostSentence),
    /// Why a quick launch opened New instead of launching (its sentence starting "Not launched."), or why a template
    /// (a shortcut's press, a sequence, a taken `tl:` row) changed nothing in this open launcher. Drawn above the search
    /// box as one plain line in the warning ink, as the box's own refusal is, where the old launcher said why a
    /// template did not apply (`view::search`), and cleared by the next press of Launch as by an edit.
    Refusal(HostSentence),
    /// Why the host could not be prepared for this launch, or why a managed checkout's preview failed once the host
    /// woke. Holds the reason only: the launcher draws it beside Launch after the mode's own lead ("Not launched." or
    /// "Restart error.").
    WakeRefusal(HostSentence),
}

/// How long a quick launch's banner stays once its templates applied, unless an edit takes it away first: the old
/// launcher's six seconds, which SPEC.md gives as "about six seconds".
pub(crate) const BANNER_SHOWS_FOR: Duration = Duration::from_secs(6);

/// What a quick launch prepared out of sight ([`prepare_unseen`]) does once its templates are applied.
pub enum Prepared {
    /// Start it, as a press of Launch would, still without drawing the launcher, with `notice` under the agent choices
    /// for when it comes back. If the draft's own check refuses it before anything is sent, the launcher is shown with
    /// that reason, as after a press.
    Launch { notice: Option<String> },
    /// Show the launcher saying `note`, with `notice` under the agent choices: the templates need more choices, or
    /// did not apply.
    Show { note: Note, notice: Option<String> },
}

/// Prepare a quick launch out of sight, from the handler of the shortcut's press: make the launcher on `draft` (New's
/// prefill), hold it in the launch slot while it reads its model catalog and the Tensorlake account, then let `finish`
/// apply the templates to its draft against that catalog and say what happens next ([`Prepared`]); a launch then still
/// waits out of sight for New's folder default or a managed checkout's preview when it needs one
/// ([`Launcher::start_or_wait_for_folder`]). SPEC.md, "Quick launch": a shortcut "starts it directly ... without showing
/// the launcher on success", through "exactly New's defaults and ordinary launch path".
///
/// The catalog read is the launcher's own, and the template rules need it (a model-only template picks the one agent
/// type that offers its model; a model the chosen agent type does not offer is refused). While it runs, the slot holds
/// the launcher as being prepared, so New, the other shortcuts and the sequence prefix treat the launcher as busy.
/// The launcher stays marked as a quick launch never shown ([`Launcher::quick_launch_unseen`]) until it is first drawn.
/// `mark` is the press: the launch's completion counts from it ("Quick launches use their dispatch as that answer"),
/// and a launcher shown after the user moved on waits behind a notice instead. A read that fails leaves the catalog
/// unread, as it does for a launcher opened by hand, and preparation goes on; a read that cannot be sent at all empties
/// the slot again, so New is not held up. The second half waits on the launcher itself ([`Launcher::after_reads`]),
/// so the slot is the launcher's only owner and a reset drops the read and the launcher with it.
///
/// It also waits for the Tensorlake account read ([`after_reads`]): a sandbox draft decided before it answers would show
/// New with "Loading Tensorlake settings…" instead of launching. Every quick launch waits, since whether the draft ends
/// in a sandbox is only known once its templates apply; the read is the helm's own state, answered without a host.
///
/// It asks every container host for a Podman check as an opened launcher does ([`open_noted`]), because a quick launch
/// may be the first thing after a helm restart to want one, and the helm has no result for a host until something
/// asks. If the hidden launch reaches Launch before that host's result is pushed, it is refused and shown with "this
/// host needs a Podman check", as the old app's was, and the launcher's place turns usable when the result arrives.
///
/// Refused, returning `false`, without a UI session, while a dialog or menu holds the keyboard (an open launcher is
/// not in the slot, so a second one would go live beside it), or while a quick launch may not start
/// (`super::entry::quick_launch_available`: the slot holds a launcher already, or a host change is in flight).
pub fn prepare_unseen(
    draft: LaunchDraft,
    mark: Mark,
    finish: impl FnOnce(&mut LaunchDraft, Catalog<'_>, &App) -> Prepared + 'static,
    window: &mut Window,
    cx: &mut App,
) -> bool {
    let Some(store) = session::current_store(cx) else {
        return false;
    };
    if layers::focus_layer_open(window, cx) || !super::entry::quick_launch_available(cx) {
        return false;
    }
    let handle = window.window_handle();
    let launcher = cx.new(|cx| {
        let mut launcher = Launcher::new(draft, &store, window, cx);
        launcher.quick_launch_unseen = true;
        launcher.after_reads = Some(Box::new(move |launcher: Entity<Launcher>, cx: &mut App| {
            prepared(launcher, mark, finish, handle, cx)
        }));
        launcher
    });
    launch::preparing(launcher.clone(), cx);
    if !read_catalog(launcher.downgrade(), cx) {
        launch::abandon_prepared(&launcher, cx);
    }
    placement::check_container_hosts(cx);
    placement::read_account(launcher.downgrade(), cx);
    true
}

/// Why a quick launch prepared out of sight opened New instead of launching into an untouched folder: the host's
/// history could not be read, so the folder it would start in is a stand-in `~`. The old launcher's words.
const QUICK_LAUNCH_FOLDER_UNREAD: &str = "Couldn't prepare quick launch: this host's recent setups could not be read, so its last folder is unknown; check the folder and launch";

impl Launcher {
    /// Stage golden queries through GPUI Kit's real edit event and the launcher's normal input subscription.
    #[cfg(test)]
    pub(crate) fn search_for_golden(&self) -> TextInputState {
        self.search.clone()
    }

    /// Go on with a quick launch prepared out of sight, its templates applied: wait for New's folder default first
    /// (SPEC_impl.md, "Launch templates": "a hidden quick launch waits for it"), then wait for a managed checkout's
    /// preview, as the old app's quick launch did (`checkout`), then start. Returns `true` while it waits out of
    /// sight; `false` when it started or refused, or needs the user. A folder the history could not settle (a failed
    /// read) needs the user: New is shown with the reason instead of launching into `~` (the old app's rule).
    fn start_or_wait_for_folder(
        &mut self,
        mark: Mark,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        // A hidden draft has no render to resolve the registration before the first host wake.
        self.refresh_custom_capabilities(cx);
        if self.begin_host_wake(SourceConsent::default(), mark, window, cx) {
            return true;
        }
        // The launcher is never drawn, so the history is asked for, and the default settled, here; only while Launch
        // waits for it, as the old app's quick launch did. A managed checkout's quick launch must not read it: the
        // answer's configuration revision would make the preview it waits for stale, and it would be shown instead.
        if self.draft.folder_default_waiting() {
            self.follow_history(cx);
            if self.settle_folder_default(cx) {
                self.sync_fields(window, cx);
            }
        }
        if self.draft.folder_default_waiting() && self.history_wanted(cx) {
            self.folder_wait = Some((mark, window.window_handle()));
            return true;
        }
        if self.draft.folder_default_unread() {
            self.note = Some(Note::Refusal(
                HostSentence::new("launcher-note-folder-unread").text(QUICK_LAUNCH_FOLDER_UNREAD),
            ));
            cx.notify();
            return false;
        }
        self.submit_or_wait_for_preview(SourceConsent::default(), mark, window, cx)
    }

    /// The chosen host's history answered: a quick launch waiting out of sight for New's folder default goes on, in
    /// its own window and after this update, since it starts the launch through the window. One that is no longer
    /// being prepared (a reset dropped it) is left alone.
    pub(super) fn history_settled_for_quick_launch(&mut self, cx: &mut Context<Self>) {
        let Some((mark, window)) = self.folder_wait.take() else {
            return;
        };
        let launcher = cx.entity();
        cx.defer(move |cx| {
            if !launch::still_preparing(&launcher, cx) {
                return;
            }
            let waiting = cx.update_window(window, |_, window, cx| {
                launcher.update(cx, |launcher, cx| {
                    launcher.start_or_wait_for_folder(mark, window, cx)
                })
            });
            if !matches!(waiting, Ok(true)) {
                launch::reveal_prepared(launcher, window, mark, cx);
            }
        });
    }
}

/// [`prepare_unseen`]'s second half, once its catalog and Tensorlake account reads have answered: apply, then start or
/// show.
fn prepared(
    launcher: Entity<Launcher>,
    mark: Mark,
    finish: impl FnOnce(&mut LaunchDraft, Catalog<'_>, &App) -> Prepared,
    window: AnyWindowHandle,
    cx: &mut App,
) {
    if !launch::still_preparing(&launcher, cx) {
        return;
    }
    let waiting = cx.update_window(window, |_, window, cx| {
        launcher.update(cx, |launcher, cx| {
            let next = finish(&mut launcher.draft, launcher.catalog.as_deref(), cx);
            // The fields were made from New's bare prefill; whatever happens next, a launcher that is drawn later (now,
            // or after the helm refuses the launch) must show the draft it holds.
            launcher.sync_fields(window, cx);
            match next {
                Prepared::Launch { notice } => {
                    launcher.notice = notice;
                    launcher.start_or_wait_for_folder(mark, window, cx)
                }
                Prepared::Show { note, notice } => {
                    if matches!(note, Note::Banner(_)) {
                        launcher.schedule_banner_expiry(cx);
                    }
                    launcher.note = Some(note);
                    launcher.notice = notice;
                    cx.notify();
                    false
                }
            }
        })
    });
    // A launch that went out moved the launcher to waiting for its reply, and one waiting for its preview stays out of
    // sight until that answers; anything else comes back to be seen.
    if !matches!(waiting, Ok(true)) {
        launch::reveal_prepared(launcher, window, mark, cx);
    }
}

/// Read the release's model catalog for `launcher`. Explicit model IDs still work after a failed read. An exact
/// family word needs the catalog to pick a concrete ID. The failure gives its reason and a Retry so the user
/// can load the catalog again without losing the draft.
///
/// Once it answers, either way, the template batches that waited for it are applied in the order they arrived, and
/// then the launcher's [`Launcher::after_reads`] runs (a quick launch's preparation), if the account has answered too
/// ([`after_reads`]). The callback holds the launcher weakly, as `operations::send` asks. Returns whether the read was
/// sent; it is not without a UI session, or if the constant path ever failed to build.
fn read_catalog(launcher: WeakEntity<Launcher>, cx: &mut App) -> bool {
    read_model_catalog(cx, move |answer, cx| {
        let waited = launcher
            .update(cx, |launcher, cx| {
                match answer {
                    Ok(catalog) => launcher.catalog = Some(catalog),
                    Err(error) => launcher.catalog_error = Some(error),
                }
                // The new model rows affect preselection. Any explicit keyboard target still follows its row.
                cx.notify();
                std::mem::take(&mut launcher.waiting_batches)
            })
            .unwrap_or_default();
        for (window, batch) in waited {
            let _ = cx.update_window(window, |_, window, cx| {
                let _ = launcher.update(cx, |launcher, cx| launcher.apply_now(batch, window, cx));
            });
        }
        if let Some(launcher) = launcher.upgrade() {
            after_reads(launcher, cx);
        }
    })
}

/// Run `launcher`'s [`Launcher::after_reads`] (a quick launch's second half) once both its reads have answered, the
/// model catalog and the Tensorlake account, each either way. Called as each one answers, so whichever comes last runs
/// it, once: it is taken as it runs.
pub(super) fn after_reads(launcher: Entity<Launcher>, cx: &mut App) {
    let after = launcher.update(cx, |launcher, _| {
        let answered = (launcher.catalog.is_some() || launcher.catalog_error.is_some())
            && !matches!(launcher.account, Account::Reading);
        if answered {
            launcher.after_reads.take()
        } else {
            None
        }
    });
    if let Some(after) = after {
        after(launcher, cx);
    }
}

/// What a launcher prepared out of sight does once its reads have answered ([`Launcher::after_reads`]).
type AfterReads = Box<dyn FnOnce(Entity<Launcher>, &mut App)>;

/// One template batch for [`Launcher::apply_batch`]: the edits it makes to the draft against the catalog, reading the
/// store through the `App` it is given, and what it leaves the launcher saying (the notice under the agent choices and
/// a note, [`BatchApplied`]), or the refusal to show instead.
type Batch = Box<dyn FnOnce(&mut LaunchDraft, Catalog<'_>, &App) -> BatchResult>;

/// What a [`Batch`] did: what it leaves the launcher saying ([`BatchApplied`]), or its refusal.
type BatchResult = Result<BatchApplied, HostSentence>;

/// What an applied [`Batch`] leaves the launcher saying: the agent choices' notice (`Some(None)` clears it, `None`
/// leaves it), and a note to show once it has applied, if any (a quick switcher pick's needs-more-choices banner), in
/// place of whatever the edit would otherwise have cleared.
pub(crate) struct BatchApplied {
    pub(crate) notice: Option<Option<String>>,
    pub(crate) note: Option<Note>,
    /// The last template in the successful batch, which the command pane marks as the form's source.
    pub(crate) template: Option<String>,
    /// Whether the batch switched the agent type, which takes an earlier escalation banner away
    /// (`view::escalation`).
    pub(crate) switched_agent: bool,
    /// The permission escalation the whole batch made, if any: the banner it puts up.
    pub(crate) escalation: Option<super::draft::Escalation>,
}

/// What the shown source warning permits each mode to stop. A create has no source.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct SourceConsent {
    stop_working: bool,
    delete: Option<DeleteGuard>,
}

/// The automatic folder New or Clone offers for an installation, with its history state and wake eligibility.
/// An unread history reply can make Launch available without changing the displayed `~`.
type DefaultFolderTarget = (Install, String, FolderDefault, bool);

/// The open launcher. See the module docs.
pub(crate) struct Launcher {
    pub(super) draft: LaunchDraft,
    source_frame: ActionFrame<SourceConsent>,
    /// A keyboard event can redraw the launcher and recover focus from a removed chip before the user sees the
    /// new focus. An Enter in that gap cannot authorize a launch from the unseen frame.
    custom_chips_frame: ActionFrame<Vec<String>>,
    default_folder_frame: ActionFrame<DefaultFolderTarget>,
    /// The store, read while drawing for the hosts, the source session and the preferences.
    store: Entity<HelmStore>,
    /// The model catalog once read; `None` until then, or after a failed read.
    catalog: Option<Vec<LaunchCatalogModel>>,
    /// Why the last catalog read failed, until a retry starts.
    catalog_error: Option<HostText>,
    /// The Tensorlake account as read when the launcher opened (`placement::read_account`), the launcher's one copy.
    account: Account,
    /// What the last agent-choice edit cleared or refused, said under the agent choices until the next one.
    notice: Option<String>,
    /// Whether the main action was pressed since the draft last changed, which shows its reason beside it.
    pressed: bool,
    /// This launcher's last launch, once the helm refused it: its key, under which the operation store keeps the
    /// refusal's words (read from the store when drawing, never copied here), and whether the reply says the create may
    /// still happen, which the store does not keep. Shown beside the main action until the draft changes or Launch is
    /// pressed again (`launch`).
    refused: Option<launch::RefusedLaunch>,
    pub(super) folder: TextInputState,
    pub(super) name: TextInputState,
    pub(super) conversation: TextInputState,
    /// Explicit expansion is retained across redraws and tabs. A field requiring attention also sets this bit so
    /// correcting that field leaves its editor and focus in place.
    more_options_open: bool,
    pub(super) wrapper: TextInputState,
    pub(super) prompt: TextInputState,
    /// The Custom permission flags, as typed.
    pub(super) flags: TextInputState,
    /// The model field's text: the chosen model while closed, what is typed while its list is open.
    pub(super) model_input: TextInputState,
    /// The model and effort of a registered integration are literal flags with no model catalog.
    custom_model: TextInputState,
    custom_effort: TextInputState,
    /// Restart with's stored command fields, used to keep their unsafe spellings read-only until Clear.
    opened: shown_text::Opened,
    /// The model field's list and what the user is doing in it.
    model_field: model_field::ModelField,
    /// The composer search box's text: what the user is searching for, emptied when they take a row.
    pub(super) search: TextInputState,
    /// The search box's open results, highlight and last refusal (`search`).
    search_box: search::SearchBox,
    /// The host picker's list, highlight and typeahead.
    host_picker: host_picker::HostPicker,
    /// The folder check: what was last asked about the folder and what the answer found (`folder_check`).
    folder_check: folder_check::FolderCheck,
    /// The folder field's dropdown of recent folders and resets (`folder_list`).
    folder_list: folder_list::FolderList,
    /// The chosen host's launch history, which the folder list and the recent setups offer again (`history`).
    history: history::LaunchHistory,
    /// The managed-checkout destination's fields, suggestions and preview (`checkout`).
    checkout: checkout::CheckoutState,
    /// The host button's focus handle; leaving it closes the host list.
    pub(super) host_focus: gpui::FocusHandle,
    /// The Command tab's command line and resume command fields; a stored value holding invisible characters shows
    /// spelled out there (`shown_text`).
    pub(super) command: TextInputState,
    pub(super) resume: TextInputState,
    /// Cancel's focus handle, where Restart with starts.
    pub(super) cancel_focus: gpui::FocusHandle,
    layer: Option<LayerId>,
    /// The YOLO question for a launch the helm refused because its host asks first, from the refusal until it is
    /// answered or the draft changes (`yolo_question`).
    yolo: Option<yolo_question::YoloState>,
    /// A settings read or question for the agent picked in this launcher. Templates have their own launch path.
    hook_question: Option<hook_question::HookQuestion>,
    /// An unanswered offer to recheck when its tab restores the same agent. Agent and Command each own their pick.
    hook_offer_on_return: [Option<crate::hook_setup::HookAgent>; 2],
    hook_pick: u64,
    /// "Create a sandbox for this session?", from the press that needs it until it is answered or the draft changes
    /// (`sandbox_question`).
    sandbox: Option<sandbox_question::SandboxState>,
    /// Whether this launcher has sent a create on the account's "Create Tensorlake sandboxes without asking", after
    /// which it asks instead (`sandbox_question`).
    sandbox_skip_spent: bool,
    /// Set while this launcher is a quick launch's that has never been drawn ([`prepare_unseen`]), and cleared for good
    /// the first time it is ([`show`]). The success path reads it to hand the new session to naming (SPEC.md, "Quick
    /// launch": only "When New was never shown"; `launch::settle`).
    pub(super) quick_launch_unseen: bool,
    /// A quick launch prepared out of sight that waits for the chosen host's history to settle New's folder default
    /// ([`Launcher::start_or_wait_for_folder`]), with the mark it started from and its window; it goes on when the
    /// history answers.
    folder_wait: Option<(Mark, AnyWindowHandle)>,
    /// The permission-escalation banner while it shows (`escalation`): its own state, which edits other than an agent
    /// switch leave alone, unlike [`Self::note`].
    escalation: Option<escalation::EscalationBanner>,
    /// How many escalation banners this launcher has shown, so each new one's motion plays afresh.
    escalation_generation: u64,
    /// Browse's last request and answer (`browse`).
    browse: browse::Browse,
    /// What the launcher says for a quick launch or a template shortcut ([`Note`]), until the next edit, or for a
    /// banner until [`Launcher::banner_expiry`] if that comes first.
    note: Option<Note>,
    /// When a quick launch's banner goes on its own ([`BANNER_SHOWS_FOR`] after its templates applied). Dropped with
    /// the launcher; an edit that takes the banner away first leaves it to find nothing to clear.
    banner_expiry: Option<ticker::Deadline>,
    /// Template batches that arrived while the catalog read was out, with the window each was pressed in, applied in
    /// order once it answers ([`Launcher::apply_batch`]). A plain queue, with no fence: the read answers once, and a
    /// reset drops the launcher with it. A press of Launch that sends its create drops what is still waiting, since those
    /// batches spoke of the draft just launched and must not edit it while its create is out.
    waiting_batches: Vec<(AnyWindowHandle, Batch)>,
    /// A quick launch's second half, set by [`prepare_unseen`] and run once both reads have answered ([`after_reads`],
    /// `prepared`): kept here rather than in a callback so the launch slot stays the hidden launcher's only owner.
    pub(super) after_reads: Option<AfterReads>,
    /// The Save as template panel, while it is open (`crate::templates::save_panel`): drawn at the end of the body,
    /// and holding the launcher's Launch, and while it saves, its Cancel, Close and Escape too.
    save_panel: Option<Entity<SavePanel>>,
    /// Why Save as template did not open (a wrapper prefix that does not parse, a retired container choice), said as a
    /// plain line in the notices region, not as a refused launch (SPEC.md: "Errors that are not a refused launch, such
    /// as "Save as template" refusing a wrapper, keep the plain line"), until the draft changes.
    save_refusal: Option<String>,
    _fields: Vec<Subscription>,
}

/// Which text field an event came from.
#[derive(Clone, Copy)]
enum Input {
    Folder,
    Name,
    Conversation,
    Wrapper,
    Prompt,
    Flags,
    Command,
    Resume,
}

impl Launcher {
    fn new(
        draft: LaunchDraft,
        store: &Entity<HelmStore>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let field = |text: &str, window: &mut Window, cx: &mut Context<Self>| {
            let state = TextInputState::new(window, cx);
            state.set_text(text.to_owned(), window, cx);
            state
        };
        let opened = shown_text::Opened::of(&draft);
        let present = |which: Input| which.present(&draft, &opened);
        let folder = field(&present(Input::Folder).text, window, cx);
        let name = field(&present(Input::Name).text, window, cx);
        let conversation = field(&draft.conversation, window, cx);
        // The mode never changes while the launcher is open, so neither does this placeholder.
        conversation.set_placeholder(
            if draft.requires_conversation() {
                "Required for this restart"
            } else {
                "Optional, this start only"
            },
            window,
            cx,
        );
        // A value holding invisible characters starts spelled out, and read-only where the old field was (`shown_text`);
        // `sync_fields` keeps it so as the draft changes.
        let wrapper = TextInputState::new(window, cx);
        wrapper.set_placeholder("with-endpoint --profile work --", window, cx);
        let prompt = TextInputState::new_growing(PROMPT_ROWS, window, cx);
        prompt.set_placeholder(prompt_placeholder(draft.agent.harness), window, cx);
        let flags = TextInputState::new(window, cx);
        let command = TextInputState::new(window, cx);
        let resume = TextInputState::new(window, cx);
        for (state, shown) in [
            (&wrapper, present(Input::Wrapper)),
            (&prompt, present(Input::Prompt)),
            (&flags, present(Input::Flags)),
            (&command, present(Input::Command)),
            (&resume, present(Input::Resume)),
        ] {
            state.set_text(shown.text, window, cx);
            state.set_read_only(shown.read_only, cx);
        }
        // The model field is not a plain text edit: what is typed filters its list, and Enter, focus and blur drive it
        // (`model_field`), so it has its own subscriptions.
        let model_input = field(&model_field::model_display(&draft), window, cx);
        let custom_model = field(draft.agent.model.as_deref().unwrap_or_default(), window, cx);
        let custom_effort = field(&draft.agent.custom_effort, window, cx);
        custom_model.set_placeholder("Agent default", window, cx);
        custom_effort.set_placeholder("Agent default", window, cx);
        // The search box is not a plain text edit either: what is typed is a query, and Enter takes a row (`search`).
        let search = TextInputState::new(window, cx);
        search.set_placeholder(
            format!(
                "{}: type an agent, host, folder, model or name…",
                search::command_title(&draft.mode)
            ),
            window,
            cx,
        );
        let search_events = search.subscribe(
            window,
            cx,
            |launcher: &mut Self, event, window, cx| match event {
                TextInputEvent::Changed => launcher.search_typed(window, cx),
                TextInputEvent::Submitted { held } => launcher.search_enter(held, window, cx),
            },
        );
        // Leaving the box (Tab, say) closes its results, as leaving the model field closes its list; deferred for the
        // focus-phase reason given below.
        let search_blur = cx.on_blur(&search.focus_handle(cx), window, |_, window, cx| {
            cx.defer_in(window, |launcher, window, cx| {
                launcher.close_search(window, cx);
                cx.notify();
            });
        });
        // The folder field's dropdown opens while the field has focus (`folder_list`), deferred for the focus-phase
        // reason given below for the model field.
        let folder_handle = folder.focus_handle(cx);
        let folder_focus = cx.on_focus(&folder_handle, window, |_, window, cx| {
            cx.defer_in(window, |launcher, window, cx| {
                launcher.folder_focused(window, cx)
            });
        });
        let folder_blur = cx.on_blur(&folder_handle, window, |_, window, cx| {
            cx.defer_in(window, |launcher, window, cx| {
                launcher.folder_blurred(window, cx)
            });
        });
        let model_handle = model_input.focus_handle(cx);
        let model_events = model_input.subscribe(
            window,
            cx,
            |launcher: &mut Self, event, window, cx| match event {
                TextInputEvent::Changed => launcher.model_typed(window, cx),
                TextInputEvent::Submitted { held: false } => launcher.model_enter(window, cx),
                // A held Enter applies nothing: the old launcher's model field ignored an auto-repeated Enter, so a key
                // held down from an earlier control cannot pick a row or a typed id on its own.
                TextInputEvent::Submitted { held: true } => {}
            },
        );
        // GPUI runs focus listeners inside the frame's focus phase, where a redraw request is dropped (`focus.rs` works
        // around the same trap for its own listener), so opening and closing the list there would show only on some
        // later frame. Each is deferred to just after, where it redraws at once.
        let model_focus = cx.on_focus(&model_handle, window, |_, window, cx| {
            cx.defer_in(window, |launcher, window, cx| {
                launcher.model_focused(window, cx)
            });
        });
        let model_blur = cx.on_blur(&model_handle, window, |_, window, cx| {
            cx.defer_in(window, |launcher, window, cx| {
                launcher.model_blurred(window, cx)
            });
        });
        // The host list closes when the host button loses focus (the old picker's blur), deferred for the same reason.
        // Tab stops, as `focus::control_handle` makes every control's: the launcher owns these two because it focuses
        // or watches them (Restart with starts on Cancel; leaving the host button closes its list).
        let host_focus = cx.focus_handle().tab_stop(true);
        let host_blur = cx.on_blur(&host_focus, window, |_, window, cx| {
            cx.defer_in(window, |launcher, window, cx| {
                launcher.close_host_list(window, cx);
                cx.notify();
            });
        });
        let mut fields: Vec<Subscription> = [
            (&folder, Input::Folder),
            (&name, Input::Name),
            (&conversation, Input::Conversation),
            (&wrapper, Input::Wrapper),
            (&prompt, Input::Prompt),
            (&flags, Input::Flags),
            (&command, Input::Command),
            (&resume, Input::Resume),
        ]
        .into_iter()
        .map(|(state, which)| {
            let input = state.clone();
            state.subscribe(window, cx, move |launcher: &mut Self, event, window, cx| {
                match event {
                    TextInputEvent::Changed => {
                        if matches!(which, Input::Folder) {
                            launcher.folder_typed(window, cx);
                        }
                        launcher.edited(which, input.text(cx).to_string(), cx);
                        // A typed or pasted invisible character can change how the field shows its value
                        // (`shown_text`), so it is put in step at once rather than at the next resync.
                        if which.shown_through_text_rules(&launcher.draft, &launcher.opened) {
                            launcher.sync_fields(window, cx);
                        }
                    }
                    // SPEC.md: "A held Enter never launches". A single-line field reports Submitted on every
                    // auto-repeat of Enter, and its `held` says which (buttons, the choices and the dialog's own
                    // Enter already ignore a held key). In the folder field, Enter on a row an arrow highlighted takes
                    // that row instead of launching (`folder_list`).
                    TextInputEvent::Submitted { held } => {
                        if held {
                            return;
                        }
                        if matches!(which, Input::Folder) && launcher.folder_enter(window, cx) {
                            return;
                        }
                        launcher.press(window, cx);
                    }
                }
            })
        })
        .collect();
        for (input, is_model) in [(&custom_model, true), (&custom_effort, false)] {
            let text = input.clone();
            fields.push(input.subscribe(
                window,
                cx,
                move |launcher: &mut Self, event, window, cx| {
                    match event {
                        TextInputEvent::Changed
                            if launcher
                                .draft
                                .agent
                                .harness
                                .is_some_and(LaunchHarness::uses_custom_catalog) =>
                        {
                            let value = text.text(cx).to_string();
                            if is_model {
                                launcher.draft.agent.model = (!value.is_empty()).then_some(value);
                                launcher.draft.agent.model_owner = launcher.draft.agent.harness;
                            } else {
                                launcher.draft.agent.custom_effort = value;
                            }
                            launcher.changed(cx);
                        }
                        TextInputEvent::Submitted { held: false } => launcher.press(window, cx),
                        _ => {}
                    }
                },
            ));
        }
        fields.extend([
            model_events,
            model_focus,
            model_blur,
            host_blur,
            search_events,
            search_blur,
            folder_focus,
            folder_blur,
        ]);
        let repository = draft
            .checkout
            .as_ref()
            .map(|checkout| checkout.field_text())
            .unwrap_or_default();
        let checkout = checkout::CheckoutState::new(&repository, window, cx);
        Launcher {
            draft,
            store: store.clone(),
            catalog: None,
            catalog_error: None,
            notice: None,
            pressed: false,
            source_frame: ActionFrame::default(),
            default_folder_frame: ActionFrame::default(),
            custom_chips_frame: ActionFrame::default(),
            refused: None,
            folder,
            name,
            conversation,
            more_options_open: false,
            wrapper,
            prompt,
            flags,
            model_input,
            custom_model,
            custom_effort,
            opened,
            model_field: model_field::ModelField::default(),
            search,
            search_box: search::SearchBox::default(),
            host_picker: host_picker::HostPicker::default(),
            folder_check: folder_check::FolderCheck::default(),
            folder_list: folder_list::FolderList::default(),
            history: history::LaunchHistory::default(),
            checkout,
            host_focus,
            command,
            resume,
            cancel_focus: cx.focus_handle().tab_stop(true),
            layer: None,
            yolo: None,
            hook_question: None,
            hook_offer_on_return: [None; 2],
            hook_pick: 0,
            sandbox: None,
            sandbox_skip_spent: false,
            quick_launch_unseen: false,
            escalation: None,
            escalation_generation: 0,
            browse: browse::Browse::default(),
            folder_wait: None,
            note: None,
            banner_expiry: None,
            waiting_batches: Vec::new(),
            after_reads: None,
            account: Account::Reading,
            save_panel: None,
            save_refusal: None,
            _fields: fields,
        }
    }

    /// Where focus starts: Cancel in Restart with, the search box otherwise, a managed checkout's included (SPEC.md:
    /// "Ordinary launches still start focus on search; Restart starts on Cancel").
    fn start_focus(&self, cx: &App) -> gpui::FocusHandle {
        match self.draft.mode {
            Mode::RestartWith(_) => self.cancel_focus.clone(),
            Mode::New | Mode::Clone | Mode::ReplaceWith { .. } => self.search.focus_handle(cx),
        }
    }

    /// The user changed field `which` to `text`: the draft's edit for it. Any edit takes the shown reason away until the
    /// next press, so the user is not told about a draft they already changed.
    fn edited(&mut self, which: Input, text: String, cx: &mut Context<Self>) {
        let text = which.edited_text(text, &self.draft, &self.opened);
        match which {
            Input::Folder => self.draft.edit_folder(text),
            Input::Name => self.draft.edit_name(text),
            Input::Conversation => self.draft.edit_conversation(text),
            Input::Wrapper => self.draft.edit_wrapper(text),
            Input::Prompt => self.draft.edit_prompt(text),
            Input::Flags => self.draft.edit_permission_flags(text),
            Input::Command => self.draft.edit_command(text),
            Input::Resume => self.draft.edit_command_resume(text),
        }
        self.changed(cx);
    }

    /// The draft changed: take the shown reason away until the next press, so the user is never told about a draft
    /// they already changed, and redraw. Every edit, whatever control made it, ends here.
    fn changed(&mut self, cx: &mut Context<Self>) {
        self.pressed = false;
        self.refused = None;
        // A YOLO question asks about the launch that was sent; once the draft changes, it would answer for another.
        self.yolo = None;
        // So does the sandbox question, about the draft at its press.
        self.sandbox = None;
        self.retire_hook_answer(cx);
        // A quick launch's banner or refusal speaks of the draft as it opened, a shortcut's refusal of the press before.
        self.note = None;
        self.save_refusal = None;
        // The guarded answer's launch waiting for a checkout preview would launch something else than was answered.
        self.disarm_checkout_launch();
        cx.notify();
    }

    /// Take a quick launch's banner away [`BANNER_SHOWS_FOR`] from now, unless an edit has already (SPEC.md, "Template
    /// number shortcuts": "The banner expires about six seconds after application finishes, or when the user starts
    /// editing"). Called once the templates applied, so a slow catalog read does not spend the time the banner is
    /// there to be read in. A refusal note never expires: it says why nothing launched, and stays until an edit.
    fn schedule_banner_expiry(&mut self, cx: &mut Context<Self>) {
        let launcher = cx.weak_entity();
        self.banner_expiry = Some(ticker::after(BANNER_SHOWS_FOR, cx, move |cx| {
            let _ = launcher.update(cx, |launcher, cx| {
                if matches!(launcher.note, Some(Note::Banner(_))) {
                    launcher.note = None;
                    cx.notify();
                }
            });
        }));
    }

    /// Apply one batch of template edits to the draft (one shortcut press or one completed sequence: SPEC.md, "Each
    /// completed sequence is one refusal unit"; or a quick switcher pick). `edit` makes the edits through the draft's
    /// own hand-edit methods (`crate::templates::apply`), given the catalog as read and the `App` to read the store
    /// from (a batch that waited for the catalog reads it then, not at the press), and returns what the launcher says
    /// afterwards ([`BatchApplied`]), or the refusal to show instead, having changed nothing.
    ///
    /// An applied batch is an edit like any other: the text fields are put back in step with the draft, and the
    /// launcher says what the batch returned (its note, if any), not what it said before. A refused one changes nothing
    /// but what it says. Focus stays where it is, so a press
    /// never moves the user out of the field they are in; a control the edits remove hands focus on through the focus
    /// coordinator, as any control that goes away does. Nothing happens while the YOLO question's host write is out or
    /// Save as template is open. The panel saves the draft it captured when opened.
    ///
    /// While the catalog read is out the batch waits for it, behind any batch already waiting, since the template rules
    /// need the catalog (the old app held shortcut batches while its catalog loaded, too).
    pub(crate) fn apply_batch(
        &mut self,
        edit: impl FnOnce(&mut LaunchDraft, Catalog<'_>, &App) -> BatchResult + 'static,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.yolo_in_flight(cx) || self.save_panel.is_some() {
            return;
        }
        if self.catalog.is_none() && self.catalog_error.is_none() {
            self.waiting_batches
                .push((window.window_handle(), Box::new(edit)));
            return;
        }
        self.apply_now(Box::new(edit), window, cx);
    }

    /// [`Launcher::apply_batch`] with the catalog answered.
    fn apply_now(&mut self, edit: Batch, window: &mut Window, cx: &mut Context<Self>) {
        // What the agent choices said before the batch, for an escalation banner's Undo to put back.
        let notice_before = self.notice.clone();
        // An open YOLO question answers for the launch a press sends. A template that leaves that launch as it was just
        // before the batch keeps the question, as the old launcher did ("while its completed request is unchanged");
        // one that changes it retires the question with every other edit (`changed`). The old launcher compared with
        // the refused request instead. Hold the live source constant across this comparison of the draft.
        let source = self.source_consent(cx);
        let asked = self
            .yolo
            .is_some()
            .then(|| self.request(false, source, cx))
            .flatten();
        let folder_before = self.draft.folder.value().clone();
        match edit(&mut self.draft, self.catalog.as_deref(), cx) {
            Ok(BatchApplied {
                notice,
                note,
                template,
                switched_agent,
                escalation,
            }) => {
                if let Some(notice) = notice {
                    self.notice = notice;
                }
                if *self.draft.folder.value() != folder_before {
                    self.drop_folder_highlight();
                }
                self.sync_fields(window, cx);
                let unchanged = asked.is_some() && asked == self.request(false, source, cx);
                let kept = if unchanged { self.yolo.take() } else { None };
                self.changed(cx);
                self.yolo = kept;
                self.follow_escalation(switched_agent, escalation, notice_before, cx);
                if let Some(name) = template {
                    self.search_box.source = Some(search::SetupSource::Template(name));
                }
                // After the edit, which takes away what the launcher was saying. A banner (a switcher pick's) goes on its
                // own after the same while as a quick launch's, counted from now, when its template has applied.
                if matches!(note, Some(Note::Banner(_))) {
                    self.schedule_banner_expiry(cx);
                }
                self.note = note;
            }
            Err(refusal) => {
                self.note = Some(Note::Refusal(refusal));
                cx.notify();
            }
        }
    }

    /// The helm refused this launcher's launch (`refused`): show the refusal the operation store keeps under its key
    /// beside the main action when the launcher is next drawn.
    pub(super) fn refused(&mut self, refused: launch::RefusedLaunch, cx: &mut Context<Self>) {
        self.pressed = false;
        // A managed checkout's create the helm proves it never accepted takes a new preview (`checkout`).
        self.checkout_refused(refused.unaccepted);
        self.refused = Some(refused);
        cx.notify();
    }

    /// The helm's words refusing this launcher's last launch, as the operation store keeps them, while they are shown.
    fn refusal_words(&self, cx: &App) -> Option<HostText> {
        let refused = self.refused.as_ref()?;
        let operations = session::current_operations(cx)?;
        operations
            .read(cx)
            .failure(&refused.key)
            .filter(|failure| failure.work == refused.work)
            .map(|failure| failure.message.clone())
    }

    /// Put the text fields the draft can change on its own back in step with it (after Reset choices, or a template
    /// batch, which can set the name, the command and its resume command too). A field already showing the draft's
    /// value is left alone, so its caret and selection stay where they were. Edits also resync fields whose spelling
    /// can change, for example when a paste introduces an invisible character or completes its spelling.
    fn sync_fields(&self, window: &mut Window, cx: &mut Context<Self>) {
        self.prompt
            .set_placeholder(prompt_placeholder(self.draft.agent.harness), window, cx);
        let present = |which: Input| which.present(&self.draft, &self.opened);
        for (state, shown) in [
            (&self.wrapper, present(Input::Wrapper)),
            (&self.prompt, present(Input::Prompt)),
            (&self.flags, present(Input::Flags)),
            (&self.command, present(Input::Command)),
            (&self.resume, present(Input::Resume)),
        ] {
            state.set_read_only(shown.read_only, cx);
            if state.text(cx).as_ref() != shown.text {
                state.set_text(shown.text, window, cx);
            }
        }
        let pairs = [
            (&self.conversation, self.draft.conversation.clone()),
            (&self.folder, present(Input::Folder).text),
            (
                &self.custom_model,
                self.draft.agent.model.clone().unwrap_or_default(),
            ),
            (&self.custom_effort, self.draft.agent.custom_effort.clone()),
            // While a managed checkout is chosen the field shows the name the checkout gets, which ignores an untouched
            // copied title.
            (&self.name, present(Input::Name).text),
            (
                &self.checkout.repository,
                self.draft
                    .checkout
                    .as_ref()
                    .map(|checkout| checkout.field_text())
                    .unwrap_or_default(),
            ),
        ];
        for (state, value) in pairs {
            if state.text(cx).as_ref() != value {
                state.set_text(value, window, cx);
            }
        }
        // The model field shows the chosen model only while its list is closed; open, it holds what is being typed.
        if !self.model_list_open(window, cx) {
            let shown = model_field::model_display(&self.draft);
            if self.model_input.text(cx).as_ref() != shown {
                self.model_input.set_text(shown, window, cx);
            }
        }
    }

    /// The hosts as the store holds them, the catalog as read, for the draft's launch check.
    fn hosts(&self, cx: &App) -> Vec<HostView> {
        self.store
            .read(cx)
            .collections()
            .ordered::<HostView>()
            .into_iter()
            .cloned()
            .collect()
    }

    /// The draft's chosen host as the store's registry holds it now; `None` when no host is chosen or the registry no
    /// longer lists it. Every view of the chosen host (its name, connection, installation, history) reads it here.
    fn chosen_host<'a>(&self, cx: &'a App) -> Option<&'a HostView> {
        let id = self.draft.host?;
        self.store
            .read(cx)
            .collections()
            .ordered::<HostView>()
            .into_iter()
            .find(|host| host.id == id)
    }

    /// The source session's stored launch as the store holds it now, for Restart with's check.
    fn live_launch(&self, cx: &App) -> Option<SessionLaunch> {
        let Mode::RestartWith(restart) = &self.draft.mode else {
            return None;
        };
        self.store
            .read(cx)
            .collections()
            .get::<SessionRow>(&restart.source)
            .map(|row| row.info.launch.clone())
    }

    /// Why the draft cannot launch now, if it cannot. A host change in flight comes before every other reason, in every
    /// mode: the draft may be complete, and it is the window, not the draft, that is not ready.
    ///
    /// Restart with also refuses while the model field holds a model not yet applied (typed into its open list, or
    /// said under it as not applicable), after the host's own reasons and before the draft's others, as the old
    /// launcher did: restarting then would quietly keep the session's old model while the field shows another.
    fn refusal(&self, cx: &App) -> Option<Refusal> {
        if crate::hosts::host_change_in_flight(cx) {
            return Some(Refusal(launch::HOST_CHANGE_IN_FLIGHT.to_owned()));
        }
        if self.source_busy(cx) {
            return Some(Refusal(SOURCE_BUSY.to_owned()));
        }
        if self.save_panel.is_some() {
            return Some(Refusal(TEMPLATE_PANEL_OPEN.to_owned()));
        }
        let hosts = self.hosts(cx);
        if let Some(reason) = self.custom_restart_refusal(cx) {
            return self
                .draft
                .host_refusal(&hosts)
                .or_else(|| Some(Refusal(reason.to_owned())));
        }
        if matches!(self.draft.mode, Mode::RestartWith(_)) && self.model_pending(cx) {
            return self
                .draft
                .host_refusal(&hosts)
                .or_else(|| Some(Refusal("Choose a model before restarting".to_owned())));
        }
        let recipes = Recipes::for_draft(&self.draft, self.store.read(cx).collections());
        let target = Target {
            hosts: &hosts,
            catalog: self.catalog.as_deref(),
            preview: self.checkout_preview(cx),
            recipes: &recipes,
            account: &self.account,
        };
        // A `gh:` query not yet answered by a pick (`checkout`), after the place's own reasons.
        if self.search_holds_repository_query(cx) {
            return self
                .draft
                .place_refusal(target)
                .or_else(|| Some(Refusal(SELECT_REPOSITORY.to_owned())));
        }
        self.draft
            .launch_refusal(target, self.live_launch(cx).as_ref())
    }

    /// A registration push can change whether this saved agent can restart while the dialog is already open.
    fn custom_restart_refusal(&self, cx: &App) -> Option<&'static str> {
        if !matches!(self.draft.mode, Mode::RestartWith(_)) || self.draft.tab != Tab::Agent {
            return None;
        }
        super::entry::custom_restart_refusal(
            self.draft.agent.custom_agent.as_deref(),
            self.store.read(cx).collections(),
        )
    }

    /// Whether the source of a Replace with or Restart with has a request in flight under its own key (a Stop, a
    /// Rename, a plain Replace from its row): the launcher's request would share that key, and the operation store
    /// admits one request per key, so Launch waits rather than being turned away after the press.
    fn source_busy(&self, cx: &App) -> bool {
        let source = match &self.draft.mode {
            Mode::ReplaceWith { source } => source,
            Mode::RestartWith(restart) => &restart.source,
            Mode::New | Mode::Clone => return false,
        };
        session::current_operations(cx).is_some_and(|operations| {
            operations
                .read(cx)
                .in_flight(&OpKey::session(source))
                .is_some()
        })
    }

    /// What Replace with says beside its main action about its source as the store shows it now, and the Delete guard a
    /// press sends ([`replace::launcher_warning`]): the warning follows the source's state while the launcher stays open,
    /// and the guard is the one matching what is drawn. A source the store no longer holds shows nothing and sends the
    /// strictest guard. `None` outside Replace with.
    fn replace_warning(&self, cx: &App) -> Option<(Option<String>, DeleteGuard)> {
        let Mode::ReplaceWith { source } = &self.draft.mode else {
            return None;
        };
        let row = self.store.read(cx).collections().get::<SessionRow>(source);
        Some(row.map_or((None, DeleteGuard::NothingAlive), replace::launcher_warning))
    }

    /// Each live warning and its answer agree on whether the agent or tabs may be stopped. Different tab counts
    /// can carry the same permission, which the supervisor checks against its current state.
    fn source_consent(&self, cx: &App) -> SourceConsent {
        SourceConsent {
            stop_working: self.source_working(cx),
            delete: self.replace_warning(cx).map(|(_, guard)| guard),
        }
    }

    /// What the main action stops of its source, as a line for the notices region, first in it as the old launcher's
    /// footer notices had it: Replace with's warning (`.launch-composer-replace-warning`), since the launch button is
    /// the confirmation of the source's delete, or Restart with's while its agent works. Either follows the source while
    /// the launcher stays open. A polite status, so a change is announced without interrupting. `None` when there is
    /// nothing to say.
    fn source_warning_line(&self, palette: &theme::Palette, cx: &App) -> Option<Stateful<Div>> {
        // Restart with's warning while its source works, in the same place and look: the old launcher's
        // `.launch-composer-restart-warning`, worded as it was.
        let warning = if self.source_working(cx) {
            "The agent is working. This action stops it before restarting.".to_owned()
        } else {
            let (warning, _) = self.replace_warning(cx)?;
            warning?
        };
        Some(
            div()
                .id("launcher-source-warning")
                .debug_selector(|| "launcher-source-warning".into())
                .role(Role::Status)
                .aria_label(SharedString::from(warning.clone()))
                .text_size(metrics::TEXT_SM)
                .text_color(theme::color(palette.warn_bright))
                .child(warning),
        )
    }

    /// The main action was pressed (the button, or Enter on the dialog, a single-line field or a choice): launch, or
    /// show why it cannot.
    ///
    /// A complete New or Clone sends its create, a complete Replace with its source's Replace with this draft's create,
    /// and a complete Restart with its source's restart with the edited launch ([`launch::start`]); each hides: the
    /// layer closes, and the launcher waits for the reply out of sight. The press is the user's answer the reply's
    /// moved-on check counts from, so this runs in the handler of that press, never deferred.
    pub(super) fn press(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.accepts_default_folder(self.default_folder_frame.current().as_ref(), cx) {
            self.press_at(None, Mark::take(cx), window, cx);
        }
    }

    /// The only folder a history answer can change without a new user edit. Replace and Restart have their own guards.
    fn default_folder_target(&self, cx: &App) -> Option<DefaultFolderTarget> {
        if self.draft.holds(Field::Host) || !self.draft.folder_is_the_default() {
            return None;
        }
        let host = self.chosen_host(cx)?;
        Some((
            Install::of(host),
            self.draft.folder.value().to_owned(),
            self.draft.folder_default,
            self.draft.can_wake(host),
        ))
    }

    /// GPUI can draw a new history default to handle Enter before it presents the frame. Both a button and a
    /// shortcut can launch into the automatic folder only after its own frame has shown it.
    fn accepts_default_folder(
        &self,
        action: Option<&ShownAction<DefaultFolderTarget>>,
        cx: &App,
    ) -> bool {
        match (action, self.default_folder_target(cx)) {
            (None, None) => true,
            (Some(action), Some(target)) => action.shown() == Some(&target),
            _ => false,
        }
    }

    /// A button or hook answer carries its own source warning. Shortcuts without a button use the current presented
    /// warning. The input mark stays with the press through any setting write.
    fn press_at(
        &mut self,
        answered_source: Option<SourceConsent>,
        mark: Mark,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.custom_chips_changed(window, cx) {
            return;
        }
        self.refresh_custom_capabilities(cx);
        // While the YOLO question's host write is out, its answer owns the launch: a press now would send a create
        // without consent beside the one the answer sends with it.
        if self.yolo_in_flight(cx) || self.hook_answer_pending(cx) {
            return;
        }
        let shown = answered_source.or_else(|| {
            self.source_frame
                .current()
                .and_then(|frame| frame.shown().copied())
        });
        let source = match (&self.draft.mode, shown) {
            (_, Some(source)) => source,
            (Mode::ReplaceWith { .. } | Mode::RestartWith(_), None)
                if self.refusal(cx).is_none() =>
            {
                return;
            }
            _ => SourceConsent::default(),
        };
        self.submit(false, None, source, mark, window, cx);
    }

    /// The request a press would send now, with `confirm_yolo` as the user's answer to the YOLO question, or `None`
    /// while the launcher refuses the press. Shared by the press ([`Self::submit`]) and by an applied template batch,
    /// which keeps an open YOLO question while this stays the same (`apply_now`).
    fn request(&self, confirm_yolo: bool, source: SourceConsent, cx: &App) -> Option<Request> {
        let hosts = self.hosts(cx);
        let recipes = Recipes::for_draft(&self.draft, self.store.read(cx).collections());
        let target = Target {
            hosts: &hosts,
            catalog: self.catalog.as_deref(),
            preview: self.checkout_preview(cx),
            recipes: &recipes,
            account: &self.account,
        };
        if self.refusal(cx).is_some() {
            None
        } else {
            match &self.draft.mode {
                // A new environment's launch goes to its own route (`LaunchDraft::environment_body`).
                Mode::New | Mode::Clone if self.draft.target.is_some() => {
                    self.draft.environment_body(target).map(|mut body| {
                        body.request.confirm_yolo = confirm_yolo;
                        Request::Environment(body)
                    })
                }
                Mode::New | Mode::Clone => self.draft.create_body(target).map(|mut body| {
                    body.confirm_yolo = confirm_yolo;
                    Request::Create(body)
                }),
                Mode::ReplaceWith { source: key } => source
                    .delete
                    .and_then(|guard| self.draft.replace_body(target, guard))
                    .map(|mut body| {
                        // The helm counts either place; the shared body's rule is to set this one.
                        body.confirm_yolo = confirm_yolo;
                        Request::Replace {
                            source: key.clone(),
                            body,
                        }
                    }),
                Mode::RestartWith(restart) => self
                    .draft
                    .restart_body(target, self.live_launch(cx).as_ref(), source.stop_working)
                    .map(|mut body| {
                        body.confirm_yolo = confirm_yolo;
                        Request::Restart {
                            source: restart.source.clone(),
                            body,
                        }
                    }),
            }
        }
    }

    /// [`Launcher::press`], with `confirm_yolo` saying whether the user consented to a YOLO launch on a host that asks
    /// first (the YOLO question's answers, `yolo_question`), and `confirmed_size` the size they agreed to in the sandbox
    /// question (`sandbox_question`). Both permissions last for this submit. `source` carries the warning's permission
    /// across any wait. `mark` is the answer the reply's moved-on check counts from ([`launch::start`]).
    fn submit(
        &mut self,
        confirm_yolo: bool,
        confirmed_size: Option<RecipeSize>,
        source: SourceConsent,
        mark: Mark,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.refresh_custom_capabilities(cx);
        self.refused = None;
        self.yolo = None;
        self.sandbox = None;
        // A press speaks for itself: a quick launch's or a shortcut's refusal said before it would hide its reason.
        self.note = None;
        if self.begin_host_wake(source, mark, window, cx) {
            return;
        }
        let hosts = self.hosts(cx);
        let mut request = self.request(confirm_yolo, source, cx);
        // A billed environment asks first ("Create a sandbox for this session?"), and its answer submits again.
        let recipes = Recipes::for_draft(&self.draft, self.store.read(cx).collections());
        if let Some(Request::Environment(body)) = request.as_mut()
            && self.hold_for_creation_question(body, &recipes, confirmed_size, &mark, window, cx)
        {
            return;
        }
        // The progress pill's host, captured now for the request's life: the registry's display name, without the
        // footer's phase, or the new environment's label (the old pill named its target the same way). A request exists
        // only for a host the refusal check found among `hosts`.
        let host = match &self.draft.target {
            Some(environment) => request
                .as_ref()
                .map(|_| crate::environment_ui::launch_target_label(environment, &hosts)),
            None => request.as_ref().and(self.chosen_host(cx)).map(display_name),
        };
        let (Some(request), Some(host)) = (request, host) else {
            self.pressed = true;
            cx.notify();
            return;
        };
        if launch::start(cx.entity(), request, host, mark, window, cx) {
            // Template batches still waiting for the catalog spoke of the draft just sent.
            self.waiting_batches.clear();
            if let Some(layer) = self.layer.take() {
                layers::close(layer, window, cx);
            }
        }
    }

    /// Reset choices: the agent choices back to New's, with the remembered permission; in Restart with, the source's
    /// own launch.
    fn reset_choices(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let remembered = self
            .store
            .read(cx)
            .collections()
            .get::<Preferences>(PREFERENCES_KEY)
            .and_then(|preferences| {
                remembered_permission(preferences.remembered_permissions.as_deref())
            });
        self.draft.reset_choices(remembered);
        self.search_box.source = None;
        self.notice = None;
        // The old Reset also emptied the search box and closed its results.
        self.search.set_text("", window, cx);
        self.close_search(window, cx);
        self.sync_fields(window, cx);
        self.changed(cx);
    }

    /// Retry: read the catalog again, its failure cleared first so the line goes while the read is out and comes back
    /// only if it fails again.
    fn retry_catalog(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        self.catalog_error = None;
        read_catalog(cx.weak_entity(), cx);
        cx.notify();
    }

    /// Clear initial prompt: the prompt emptied, as the user could by selecting it all and deleting.
    fn clear_prompt(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.draft.edit_prompt(String::new());
        self.sync_fields(window, cx);
        self.changed(cx);
    }

    /// Cancel and Close: close the launcher, discarding its draft.
    ///
    /// Refused while the YOLO question's host write is out, as Escape is (the dialog's dismissal hook): closing then
    /// would let the write change the host's setting with nothing left to say whether it did. Refused too while Save as
    /// template's save is out, as the old launcher's Cancel was, so its refusal can still be said in the panel.
    fn cancel(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.yolo_in_flight(cx) || self.hook_write_in_flight(cx) || self.template_saving(cx) {
            return;
        }
        if let Some(layer) = self.layer.take() {
            layers::close(layer, window, cx);
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// Drawing
// ---------------------------------------------------------------------------------------------------------------------

/// A click handler that runs `act` on the launcher, if it is still open.
fn on_launcher(
    launcher: WeakEntity<Launcher>,
    act: impl Fn(&mut Launcher, &mut Window, &mut Context<Launcher>) + 'static,
) -> impl Fn(&ClickEvent, &mut Window, &mut App) + 'static {
    move |_, window, cx| {
        let _ = launcher.update(cx, |launcher, cx| act(launcher, window, cx));
    }
}

impl Launcher {
    /// A text field of the launcher's, as [`TextInput::new`] makes one, where, while Save as template's panel is
    /// closed, the quick-launch sequence prefix pressed with nothing selected begins a sequence (SPEC.md, "Quick-launch
    /// sequences": "In the launcher's own text fields it starts a sequence only when no text is selected; with a
    /// selection it keeps Cut"; [`TextInput::on_unselected_prefix`]). The prefix's own action is dispatched from the
    /// field, so it reaches the main view's handler as the chord does from any control that is not a text field, and
    /// arms the launcher's form of the sequence, which applies to this draft. Every field the launcher draws is made
    /// here, so none keeps the prefix while the panel is closed.
    ///
    /// While the panel is open the fields keep the chord, as every other text field does: the old launcher let them
    /// keep Cut then, and SPEC.md has a template shortcut consumed without changing the draft while a template is
    /// being saved.
    pub(super) fn launcher_input(
        &self,
        id: impl Into<SharedString>,
        state: &TextInputState,
        label: impl Into<SharedString>,
        help: HoverHelp,
    ) -> TextInput {
        let input = TextInput::new(id, state, label, help);
        if self.save_panel.is_some() {
            return input;
        }
        input.on_unselected_prefix(|window, cx| {
            window.dispatch_action(Box::new(crate::actions::BeginSequence), cx)
        })
    }
}

/// A launcher field keeps its label above the control, leaving the control the full width of its column.
/// `selector` names the whole field in tests and docs captures so a crop includes the label and any note beside it.
fn row(
    selector: &'static str,
    label: impl IntoElement,
    control: impl IntoElement,
    palette: &theme::Palette,
) -> Stateful<Div> {
    div()
        .id(selector)
        .debug_selector(move || selector.into())
        .flex()
        .flex_col()
        .gap(px(LABEL_GAP))
        .min_w_0()
        .child(field_label(label, palette))
        .child(div().min_w_0().child(control))
}

/// Shared label ink for both ordinary fields and the separately aligned parts of the address field.
fn field_label(label: impl IntoElement, palette: &theme::Palette) -> Div {
    div()
        .text_size(metrics::TEXT_SM)
        .text_color(theme::color(palette.fg_1))
        .child(label)
}

/// Draw the controls that share the first destination row under one border. The second segment holds the folder,
/// repository or recipe, with its label above the segment. Keep the component IDs stable for the bridge.
fn address_line(
    host: AnyElement,
    right: AnyElement,
    right_label: &'static str,
    right_id: &'static str,
    browse: Option<Button>,
    ink: theme::Token,
    palette: &theme::Palette,
) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(px(LABEL_GAP))
        .child(
            div()
                .flex()
                .child(field_label("Host", palette).w(px(ADDRESS_HOST_WIDTH)))
                .child(field_label(right_label, palette).px(px(8.))),
        )
        .child(
            div()
                .id("launcher-address")
                .debug_selector(|| "launcher-address".into())
                .flex()
                .items_center()
                .min_h(ADDRESS_HEIGHT)
                .border_1()
                .border_color(theme::color(ink))
                .rounded(metrics::RADIUS_SM)
                .bg(theme::color(palette.well))
                .child(
                    div()
                        .id("launcher-host-row")
                        .debug_selector(|| "launcher-host-row".into())
                        .w(px(ADDRESS_HOST_WIDTH))
                        .flex_none()
                        .border_r_1()
                        .border_color(theme::color(palette.border_dim))
                        .child(host),
                )
                .child(
                    div()
                        .id(right_id)
                        .debug_selector(move || right_id.into())
                        .flex_1()
                        .min_w_0()
                        .child(right),
                )
                .children(browse.map(|browse| {
                    div()
                        .flex_none()
                        .border_l_1()
                        .border_color(theme::color(palette.border_dim))
                        .child(browse)
                })),
        )
}

/// Put brief guidance beside the label while the control keeps its width.
fn label_with_note(label: &'static str, note: impl IntoElement, palette: &theme::Palette) -> Div {
    div()
        .flex()
        .flex_wrap()
        .items_baseline()
        .gap(px(4.))
        .child(format!("{label} ·"))
        .child(div().text_color(theme::color(palette.fg_2)).child(note))
}

/// A template, search pick or harness chip can change who will receive the prompt while the launcher stays open.
fn prompt_placeholder(harness: Option<LaunchHarness>) -> String {
    let name = harness.map_or("the agent", harness_label);
    format!("What should {name} start on?")
}

/// A quiet line of help under a control.
///
/// It is a labelled node named by its words, since plain text is not in the accessibility tree and a screen reader
/// would otherwise never hear it. Its id is its words: no two help lines of one launcher say the same thing.
fn help_line(text: impl Into<SharedString>, palette: &theme::Palette) -> Stateful<Div> {
    labelled_note(text).text_color(theme::color(palette.fg_1))
}

/// Keep a standalone sentence accessible wherever the form draws it, including beside a field label.
fn labelled_note(text: impl Into<SharedString>) -> Stateful<Div> {
    let text: SharedString = text.into();
    div()
        .id(ElementId::Name(text.clone()))
        .role(Role::Label)
        .aria_label(text.clone())
        .text_size(metrics::TEXT_SM)
        .child(text)
}

/// The quiet ink and size shared by ordinary help and help that names a host value.
fn help_line_container(palette: &theme::Palette) -> Div {
    div()
        .text_size(metrics::TEXT_SM)
        .text_color(theme::color(palette.fg_1))
}

/// The small heading for a group of launcher fields. The card supplies the outline.
fn section_label(text: &'static str, palette: &theme::Palette) -> Div {
    div()
        .text_size(px(11.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(theme::color(palette.fg_1))
        .child(text)
}

/// Restart holds its launch kind. Keep the name and mark in the form without offering another editable choice.
fn held_harness(
    harness: LaunchHarness,
    name: crate::host_text::Line,
    palette: &theme::Palette,
) -> AnyElement {
    let mark = crate::harness::glyph(harness);
    div()
        .id("launcher-held-harness")
        .role(Role::Label)
        .aria_label(format!("Agent: {}", name.accessible_text()))
        .flex()
        .items_center()
        .gap(px(6.))
        .text_size(metrics::TEXT_MD)
        .child(
            svg()
                .path(crate::harness::icon(mark).path())
                .size(px(14.))
                .text_color(theme::color(
                    crate::harness::ink(mark, palette).unwrap_or(palette.fg_1),
                )),
        )
        .child(name)
        .into_any_element()
}

/// What the main action would start, phrased for a screen reader.
struct TargetWords {
    /// Whether the Command tab is chosen, which the words open with.
    command: bool,
    /// The agent type, if one is named.
    harness: Option<&'static str>,
    custom_agent: Option<HostText>,
    /// The host's name (see [`Launcher::target`]).
    host: HostText,
    folder: HostText,
}

impl TargetWords {
    /// The words as one sentence, the host and folder escaped: "Command · Codex · host · folder".
    fn sentence(&self, id: &'static str) -> HostSentence {
        let mut sentence = HostSentence::new(id);
        if self.command {
            sentence = sentence.text("Command · ");
        }
        if let Some(custom_agent) = &self.custom_agent {
            sentence = sentence.host(custom_agent).text(" · ");
        } else if let Some(harness) = self.harness {
            sentence = sentence.text(format!("{harness} · "));
        }
        sentence.host(&self.host).text(" · ").host(&self.folder)
    }
}

impl Launcher {
    /// The main action's visible words: Launch, Replace or Restart, and "Stop and restart" while Restart with's source
    /// reads working (SPEC.md, "Restart with": its primary action reads "Restart", or "Stop and restart" while the shown
    /// agent is working).
    fn verb(&self, cx: &App) -> &'static str {
        match self.draft.mode {
            Mode::RestartWith(_) if self.source_working(cx) => "Stop and restart",
            Mode::RestartWith(_) => "Restart",
            Mode::ReplaceWith { .. } => "Replace",
            Mode::New | Mode::Clone => "Launch",
        }
    }

    /// Whether Restart with's source reads working (status Running) in the store now: then its main action is "Stop and
    /// restart", the warning beside it says the agent is stopped first, and a press is the user's consent to that stop
    /// (the request's `stop_if_running`), as the plain Restart's question is. A live agent in any other status is
    /// stopped without asking, by the supervisor's own rule. `false` outside Restart with.
    fn source_working(&self, cx: &App) -> bool {
        let Mode::RestartWith(restart) = &self.draft.mode else {
            return false;
        };
        self.store
            .read(cx)
            .collections()
            .get::<SessionRow>(&restart.source)
            .is_some_and(|row| row.info.status == SessionStatus::Running)
    }

    /// The target named by the main action: "Command" on that tab, the selected or declared agent, the host and the
    /// folder. Use the registration's name when available. Names and folders from the host are shown escaped.
    ///
    /// Restart with names the source session's own host name, as the old footer did. Otherwise a host that is not
    /// chosen, or that the store no longer lists, is "Unavailable host", the old footer's words.
    ///
    /// A registered host that is not connected carries its phase in parentheses ("host1 (Unreachable, retrying)"), as
    /// the old footer and host list did.
    fn target(&self, cx: &App) -> TargetWords {
        let collections = self.store.read(cx).collections();
        let (harness, custom_agent) = match self.draft.tab {
            Tab::Agent => (
                self.draft.agent.harness,
                self.draft.agent.custom_agent.as_deref(),
            ),
            Tab::Command => (
                self.draft.command.agent,
                self.draft.command.custom_agent.as_deref(),
            ),
        };
        let source = match &self.draft.mode {
            Mode::RestartWith(restart) => collections
                .get::<SessionRow>(&restart.source)
                .map(|row| HostText::from_host(row.host_name.as_str())),
            Mode::New | Mode::Clone | Mode::ReplaceWith { .. } => None,
        };
        let registered = self.chosen_host(cx).map(host_name_with_phase);
        // A new environment is named as the old footer named it, "new <recipe> container on <host>".
        let environment = self.draft.target.as_ref().map(|target| {
            let hosts: Vec<HostView> = collections
                .ordered::<HostView>()
                .into_iter()
                .cloned()
                .collect();
            let label = crate::environment_ui::launch_target_label(target, &hosts);
            HostText::from_host(format!("new {}", label.raw()))
        });
        let host = environment
            .or(source)
            .or(registered)
            .unwrap_or_else(|| HostText::from_host("Unavailable host"));
        TargetWords {
            command: self.draft.tab == Tab::Command,
            harness: harness.map(harness_label),
            custom_agent: custom_agent.map(|id| crate::harness::custom_name(id, collections)),
            host,
            folder: self
                .checkout_summary(cx)
                .unwrap_or_else(|| HostText::from_host(self.draft.folder.value().as_str())),
        }
    }

    /// The main action's accessible name: the operation and its target, as the old one had ("Launch Codex · Local (this
    /// machine) · ~/src"), so a screen reader hears what a press would start, not only "Launch".
    fn action_name(&self, target: &TargetWords, cx: &App) -> SharedString {
        let sentence = target.sentence("launcher-action-name");
        format!("{} {}", self.verb(cx), sentence.accessible_name()).into()
    }

    /// The Agent tab starts with its harness and model choices. What appears below depends on the harness and the
    /// mode. Restart with holds the agent type and prompt, and Custom permissions adds an editor for its flags.
    fn agent_fields(
        &mut self,
        model: Option<Vec<AnyElement>>,
        palette: &theme::Palette,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let mut fields = Vec::new();
        let harness = self.draft.agent.harness;
        if self.draft.holds(Field::AgentType) {
            fields.extend(harness.map(|harness| {
                let label = self.draft.agent.custom_agent.as_deref().map_or_else(
                    || harness_label(harness).into(),
                    |id| {
                        HostSentence::new("launcher-held-custom-name")
                            .host(&crate::harness::custom_name(
                                id,
                                self.store.read(cx).collections(),
                            ))
                            .into()
                    },
                );
                held_harness(harness, label, palette)
            }));
        } else {
            fields.push(self.harness_choices(palette, window, cx).into_any_element());
        }
        let custom = harness.is_some_and(LaunchHarness::uses_custom_catalog);
        if !custom && let Some(error) = &self.catalog_error {
            fields.push(self.catalog_error_line(error, palette, cx));
        }
        if custom {
            fields.extend(self.custom_agent_fields(palette, cx));
        } else if model.is_some() || !harness.is_some_and(LaunchHarness::is_shell) {
            fields.push(self.model_choices(model, palette, cx).into_any_element());
        }
        let prompt_offered = harness
            .filter(|harness| harness.supports_initial_prompt(false))
            .filter(|_| !self.draft.holds(Field::InitialPrompt));
        if let Some(harness) = prompt_offered {
            fields.push(
                row(
                    "launcher-prompt-row",
                    label_with_note(
                        "Initial prompt",
                        labelled_note(format!(
                            "sent once, as {}'s first message",
                            harness_label(harness)
                        )),
                        palette,
                    ),
                    self.launcher_input(
                        "launcher-prompt",
                        &self.prompt,
                        "Initial prompt",
                        HoverHelp::tooltip(
                            "Initial prompt: sent once when this session starts. Restart, Clone and Replace do not send it again.",
                        ),
                    ),
                    palette,
                )
                .into_any_element(),
            );
            // The old line under a read-only prompt (`shown_text`), saying why it cannot be edited and what to do.
            if Input::Prompt.present(&self.draft, &self.opened).read_only {
                fields.push(help_line(PROMPT_SPELLED_OUT, palette).into_any_element());
            }
            if !self.draft.prompt.is_empty() {
                fields.push(
                    div()
                        .child(
                            Button::new(
                                "launcher-clear-prompt",
                                "Clear initial prompt",
                                HoverHelp::tooltip("Clear initial prompt: remove this first message from the draft"),
                                Role::Button,
                            )
                            .tier(Tier::Neutral)
                            .on_click(on_launcher(cx.weak_entity(), Launcher::clear_prompt)),
                        )
                        .into_any_element(),
                );
            }
        }
        if self.draft.agent.permissions == Some(LaunchPermission::Custom) {
            fields.push(self.flags_editor(harness, palette));
        }
        if let Some(notice) = &self.notice {
            fields.push(
                choices::status_line("launcher-choice-notice", notice.clone(), palette)
                    .into_any_element(),
            );
        }
        if let Some(note) = harness.and_then(agent_note) {
            fields.push(help_line(note, palette).into_any_element());
        }
        fields.extend(self.more_options(harness, palette, cx));
        fields
    }

    /// "Model catalog unavailable", with the read's own words, beside Retry. The words are a live region of their
    /// own and the button beside it, so a screen reader announces the failure rather than the control.
    fn catalog_error_line(
        &self,
        error: &HostText,
        palette: &theme::Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let name = HostSentence::new("launcher-catalog-error-name")
            .text("Model catalog unavailable: ")
            .host(error)
            .accessible_name();
        div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap(px(8.))
            .text_size(metrics::TEXT_SM)
            .text_color(theme::color(palette.warn))
            .child(
                div()
                    .id("launcher-catalog-error")
                    .role(Role::Status)
                    .aria_label(name)
                    .flex()
                    .min_w_0()
                    .child("Model catalog unavailable: ")
                    .child(host_text::element(error)),
            )
            .child(
                Button::new(
                    "launcher-catalog-retry",
                    "Retry",
                    HoverHelp::tooltip("Retry: fetch Farhelm's model catalog again"),
                    Role::Button,
                )
                .tier(Tier::Neutral)
                .on_click(on_launcher(cx.weak_entity(), Launcher::retry_catalog)),
            )
            .into_any_element()
    }

    /// The Command tab's fields: the command line, the YOLO assertion, and Resume with its command when an agent type
    /// is declared.
    fn command_fields(&self, palette: &theme::Palette, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let weak = cx.weak_entity();
        let mut fields = vec![
            row(
                "launcher-command-row",
                "Agent command",
                self.launcher_input(
                    "launcher-command",
                    &self.command,
                    "Agent command",
                    HoverHelp::tooltip(
                        "Agent command: the command line Farhelm runs through Bash, as written",
                    ),
                )
                .monospace(),
                palette,
            )
            .into_any_element(),
        ];
        if self.draft.holds(Field::AgentType) {
            fields.splice(
                0..0,
                self.draft.command.agent.map(|harness| {
                    let label = self.draft.command.custom_agent.as_deref().map_or_else(
                        || harness_label(harness).into(),
                        |id| {
                            HostSentence::new("launcher-held-custom-command")
                                .host(&crate::harness::custom_name(
                                    id,
                                    self.store.read(cx).collections(),
                                ))
                                .into()
                        },
                    );
                    held_harness(harness, label, palette)
                }),
            );
        }
        if Input::Command.present(&self.draft, &self.opened).read_only {
            fields.push(shown_text::clear_button(
                "launcher-clear-command",
                "Clear agent command",
                "Clear agent command: remove this stored command before editing",
                Launcher::clear_command,
                cx,
            ));
        }
        fields.extend(self.changed_marker(Changed::Command, palette));
        let (on_yolo, on_enter) = (weak.clone(), weak.clone());
        fields.push(
            row(
                "launcher-yolo-row",
                "Runs in YOLO mode",
                RadioGroup::new(
                    "launcher-yolo",
                    "Runs in YOLO mode",
                    HoverHelp::tooltip("Whether this command bypasses its safety checks; Farhelm believes your answer"),
                    Role::RadioGroup,
                )
                .radios()
                .option(RadioOption::new(
                    "Yes (YOLO)",
                    HoverHelp::tooltip(
                        "Yes: you assert this command bypasses human, classifier and sandbox safety checks; Farhelm \
                         marks it YOLO and does not check",
                    ),
                    true,
                ))
                .option(RadioOption::new(
                    "No",
                    HoverHelp::tooltip(
                        "No: you assert this command retains a safety check that may refuse or ask you; Farhelm does \
                         not check",
                    ),
                    false,
                ).disabled(self.draft.command.agent.is_some_and(LaunchHarness::uses_custom_catalog)))
                .chosen(self.draft.command.effective_yolo())
                .on_change(move |yolo, _, cx| {
                    let _ = on_yolo.update(cx, |launcher, cx| {
                        launcher.draft.choose_command_yolo(*yolo);
                        launcher.changed(cx);
                    });
                })
                .on_enter(move |window, cx| {
                    let _ = on_enter.update(cx, |launcher, cx| launcher.press(window, cx));
                }),
                palette,
            )
            .into_any_element(),
        );
        fields.extend(self.changed_marker(Changed::Yolo, palette));
        // The declared agent type (old `.launch-command-agent`): which agent the command runs, if any, so Farhelm can
        // track and resume it. Restart with holds it, so there it is not drawn; the footer still names it.
        if !self.draft.holds(Field::AgentType) {
            let (on_agent, on_agent_enter) = (weak.clone(), weak.clone());
            let collections = self.store.read(cx).collections();
            let options = std::iter::once(SelectOption::new(
                NO_AGENT,
                "None",
                HoverHelp::tooltip(
                    "None: declare no agent type; Farhelm does not track or resume the command's agent",
                ),
            ))
            .chain(
                LaunchHarness::BUILT_INS
                    .iter()
                    .filter(|harness| !harness.is_shell())
                    .map(|harness| {
                        let label = harness_label(*harness);
                        SelectOption::new(
                            label,
                            label,
                            HoverHelp::tooltip(format!("{label}: the command runs {label}")),
                        )
                    }),
            )
            .chain(std::iter::once(SelectOption::new(
                harness_label(LaunchHarness::Custom), "Custom",
                HoverHelp::tooltip("Declare a custom agent with the command you supplied"),
            )))
            .chain(collections.ordered::<CustomAgent>().into_iter()
                .map(|agent| {
                    let label = HostSentence::new("declared-custom-agent")
                        .host(&HostText::from_host(agent.display_name.as_str()));
                    SelectOption::new(
                        format!("custom:{}", agent.id),
                        label.accessible_name(),
                        HoverHelp::tooltip(format!("Declare custom agent {}", agent.id)),
                    ).mark(crate::assets::Icon::HarnessCustom)
                }));
            let chosen = self.draft.command.custom_agent.as_ref().map_or_else(
                || {
                    self.draft
                        .command
                        .agent
                        .map_or(NO_AGENT, harness_label)
                        .to_owned()
                },
                |id| format!("custom:{id}"),
            );
            let agent_select = Select::new(
                "launcher-command-agent",
                "Agent type",
                HoverHelp::tooltip(
                    "The agent this command runs, if any, so Farhelm can track and resume it",
                ),
                Role::ComboBox,
            )
            .options(options)
            .chosen(chosen);
            // Keep the deleted ID visible so the user can repair the choice without losing the command text.
            let agent_select = match self.draft.command.custom_agent.as_deref() {
                Some(id) if collections.get::<CustomAgent>(id).is_none() => agent_select.retained(
                    HostSentence::new("retained-custom-agent")
                        .host(&HostText::from_host(id))
                        .accessible_name(),
                    HoverHelp::tooltip(
                        "Choose Custom to run this command without the deleted registration",
                    ),
                ),
                _ => agent_select,
            };
            fields.push(
                row(
                    "launcher-command-agent-row",
                    "Agent type",
                    agent_select
                        .on_choose(move |value, window, cx| {
                            let agent = LaunchHarness::ALL.iter().copied().find(|harness| {
                                !harness.is_shell() && harness_label(*harness) == value.as_ref()
                            });
                            let _ = on_agent.update(cx, |launcher, cx| {
                                if let Some(id) = value.strip_prefix("custom:") {
                                    launcher
                                        .draft
                                        .choose_command_agent(Some(LaunchHarness::Custom));
                                    launcher.draft.command.custom_agent = Some(id.to_owned());
                                } else {
                                    launcher.draft.choose_command_agent(agent);
                                }
                                launcher.changed(cx);
                                launcher.check_hook_question(window, cx);
                            });
                        })
                        .on_enter(move |window, cx| {
                            let _ = on_agent_enter
                                .update(cx, |launcher, cx| launcher.press(window, cx));
                        }),
                    palette,
                )
                .into_any_element(),
            );
        }
        if self.draft.command.agent.is_some() {
            fields.push(
                help_line(
                    "Put {farhelm_args} in the command where Farhelm adds its own arguments.",
                    palette,
                )
                .into_any_element(),
            );
            if !matches!(self.draft.mode, Mode::RestartWith(_)) {
                let (on_resume, on_resume_enter) = (weak.clone(), weak.clone());
                fields.push(
                    div()
                        .child(
                            Checkbox::new(
                                "launcher-resume",
                                "Resume",
                                HoverHelp::tooltip(
                                    "Resume: give the command a way to pick its conversation back up after a restart",
                                ),
                                Role::CheckBox,
                            )
                            .checked(self.draft.command.resume_on)
                            .on_toggle(move |on, _, cx| {
                                let _ = on_resume.update(cx, |launcher, cx| {
                                    launcher.draft.choose_command_resume(on);
                                    launcher.changed(cx);
                                });
                            })
                            .on_enter(move |window, cx| {
                                let _ =
                                    on_resume_enter.update(cx, |launcher, cx| launcher.press(window, cx));
                            }),
                        )
                        .into_any_element(),
                );
            }
            if self.draft.command.resume_on {
                fields.push(
                    row(
                        "launcher-resume-command-row",
                        "Resume command",
                        self.launcher_input(
                            "launcher-resume-command",
                            &self.resume,
                            "Resume command",
                            HoverHelp::tooltip(
                                "Resume command: how Farhelm restarts this command on its conversation; needs \
                                 {conversation} and {farhelm_args}",
                            ),
                        )
                        .monospace(),
                        palette,
                    )
                    .into_any_element(),
                );
                if Input::Resume.present(&self.draft, &self.opened).read_only {
                    fields.push(shown_text::clear_button(
                        "launcher-clear-resume",
                        "Clear resume command",
                        "Clear resume command: remove this stored resume command before editing",
                        Launcher::clear_resume,
                        cx,
                    ));
                }
            }
            fields.extend(self.changed_marker(Changed::Resume, palette));
        }
        fields
    }
}

impl Launcher {
    /// Refresh after catalog pushes and before dispatch, including template launches prepared out of sight.
    fn refresh_custom_capabilities(&mut self, cx: &App) {
        let collections = self.store.read(cx).collections();
        let selected_custom = self
            .draft
            .agent
            .custom_agent
            .as_deref()
            .and_then(|id| collections.get::<CustomAgent>(id));
        self.draft.agent.custom_resumption =
            selected_custom.is_some_and(|agent| agent.supports_resumption);
        self.draft.agent.custom_reporting =
            selected_custom.is_some_and(|agent| agent.supports_reporting);
        self.draft.agent.custom_registration_missing =
            self.draft.agent.custom_agent.is_some() && selected_custom.is_none();
        self.draft.command.custom_registration_missing = self
            .draft
            .command
            .custom_agent
            .as_deref()
            .is_some_and(|id| collections.get::<CustomAgent>(id).is_none());
    }
}

impl Render for Launcher {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.refresh_custom_capabilities(cx);
        // The folder check follows the folder and the host's connection as drawn (`folder_check`), and a managed
        // checkout's reads follow it the same way (`checkout`).
        self.follow_folder_check(cx);
        self.follow_browse(cx);
        self.follow_checkout(window, cx);
        // The helm's history of new environments is read on the first draw, and the chosen host's launch history
        // follows its connection the same way (`history`); the folder list forgets a list the layer manager closed on
        // its own, with its highlight (`folder_list`).
        self.follow_environment_history(cx);
        self.follow_history(cx);
        if self.settle_folder_default(cx) {
            self.sync_fields(window, cx);
        }
        let folder_action = self
            .default_folder_target(cx)
            .map(|target| self.default_folder_frame.capture(target, (), window));
        if folder_action.is_none() {
            self.default_folder_frame.clear();
        }
        self.follow_folder_list(window, cx);
        self.reflow_search(window, cx);
        let palette = theme::palette(cx);
        let weak = cx.weak_entity();
        let refusal = self.refusal(cx);
        let target = self.target(cx);
        // While the YOLO question's host write is out, the launcher holds still (`yolo_question`).
        let holding = self.yolo_in_flight(cx) || self.hook_write_in_flight(cx);
        let restart = matches!(self.draft.mode, Mode::RestartWith(_));
        let source = self.source_consent(cx);
        let source_action = if restart || matches!(self.draft.mode, Mode::ReplaceWith { .. }) {
            if let Some(state) = &self.hook_question {
                state.show_source(source, cx);
            }
            Some(self.source_frame.capture(source, (), window))
        } else {
            None
        };
        if let Some(state) = &self.yolo {
            state.show_request(source, self.request(false, source, cx), cx);
        }

        // A held destination (Restart with's) is shown, not edited: its folder as read-only text, its name not at all
        // (SPEC.md, "Restart with": held fields' "editing controls are hidden", and "The held agent type, host and
        // folder stay visible").
        // A new environment's folder is its home, shown as a held destination is (SPEC.md: "Folder is fixed to the new
        // sandbox's home, without Browse or recent folders").
        let managed_checkout = self.draft.checkout.is_some();
        let folder = if managed_checkout {
            None
        } else if self.draft.holds(Field::Destination) || self.draft.target.is_some() {
            Some(
                div()
                    .flex()
                    .items_center()
                    .min_h(ADDRESS_HEIGHT - px(2.))
                    .px(px(8.))
                    .font_family(crate::assets::TERMINAL_FONT)
                    .text_size(metrics::TEXT_MD)
                    .child(
                        HostSentence::new("launcher-folder-held")
                            .host(&HostText::from_host(self.draft.folder.value().as_str())),
                    )
                    .into_any_element(),
            )
        } else {
            // The folder list opens under the field (`folder_list`), as wide as it.
            let tracker = self
                .folder_list
                .list
                .track_anchor("launcher-folder-anchor", window, cx);
            let on_arrow = weak.clone();
            Some(div()
                .relative()
                .child(
                    self.launcher_input(
                        "launcher-folder",
                        &self.folder,
                        "Folder",
                        HoverHelp::tooltip(
                            "Folder: an existing directory on the host, absolute or starting with ~",
                        ),
                    )
                    .embedded()
                    .height(ADDRESS_HEIGHT - px(2.))
                    .monospace()
                    .on_vertical(move |arrow, window, cx| {
                        let _ = on_arrow
                            .update(cx, |launcher, cx| launcher.folder_arrow(arrow, window, cx));
                    }),
                )
                .child(tracker)
                .into_any_element())
        };
        // An unnamed managed checkout's name field reserves the placeholder for the `<repo>-N` it will get.
        let placeholder = if self.draft.checkout.is_none() {
            "Farhelm names it".to_owned()
        } else {
            self.checkout_name_placeholder(cx)
        };
        if placeholder != self.checkout.name_placeholder {
            self.name.set_placeholder(placeholder.clone(), window, cx);
            self.checkout.name_placeholder = placeholder;
        }
        let mut name = if self.draft.holds(Field::Name) {
            None
        } else {
            // In a managed checkout, a launch template's base name says so beside the label and, once taken, under
            // the field (`crate::templates::checkout_name`).
            let template_base = checkout_name::template_base(&self.draft);
            let caption = label_with_note("Name", "optional", palette).children(
                template_base
                    .as_ref()
                    .map(|_| checkout_name::pill(palette, window, cx)),
            );
            let taken = template_base
                .filter(|(_, taken)| *taken)
                .map(|(base, _)| checkout_name::taken_note(&base, palette, window, cx));
            let field = div()
                .flex()
                .flex_col()
                .gap(px(6.))
                // Named for tests, which find the field by where it is drawn: the row is taller than the field while
                // the pill wraps under its label.
                .child(
                    div().debug_selector(|| "launcher-name".into()).child(
                        self.launcher_input(
                            "launcher-name",
                            &self.name,
                            "Name (optional)",
                            HoverHelp::tooltip(
                                "Name: the session's title; left empty, Farhelm names it",
                            ),
                        )
                        .height(ADDRESS_HEIGHT),
                    ),
                )
                .children(taken);
            Some(
                row("launcher-name-row", caption, field, palette)
                    .flex_basis(px(DESTINATION_NAME_WIDTH))
                    .min_w(px(DESTINATION_NAME_WIDTH))
                    .flex_grow(0.3),
            )
        };
        // Host always has a segment. Recipe owns the other segment in a new environment, so Repository gets its
        // own row there. On an existing host Repository and Folder use the same slot.
        let environment = self.draft.target.clone();
        let destination_switch = self.destination_switch(cx);
        let host = self.host_control(palette, window, cx);
        let address = if let Some(target) = &environment {
            let controls = self.placement_controls(target, cx);
            let first_line = controls.recipe.map(|recipe| {
                address_line(
                    host,
                    recipe.address_segment().into_any_element(),
                    "Recipe",
                    "launcher-recipe-row",
                    None,
                    palette.border,
                    palette,
                )
            });
            let container_host = controls.container_host.map(|control| {
                div()
                    .flex()
                    .flex_col()
                    .gap(px(LABEL_GAP))
                    // Folder places this block in a horizontal row. A basis
                    // in the checkout stack would set its height.
                    .when(folder.is_some(), |block| {
                        block
                            .flex_grow_1()
                            .flex_basis(px(CONTAINER_HOST_MIN_WIDTH))
                            .min_w(px(CONTAINER_HOST_MIN_WIDTH))
                    })
                    .child(row(
                        "launcher-container-host-row",
                        "Container host",
                        control,
                        palette,
                    ))
                    .children(self.place_note(target, palette, cx))
            });
            let rows = div()
                .flex()
                .flex_col()
                .gap(px(ROW_GAP))
                .children(first_line);
            if let Some(folder) = folder {
                // A new environment starts at its home. The short, held Folder segment leaves the host menu room
                // for its reasons and keeps the same edge and height as the editable fields around it.
                let held = div()
                    .bg(theme::color(palette.well))
                    .border_1()
                    .border_color(theme::color(palette.border))
                    .rounded(metrics::RADIUS_SM)
                    .child(folder);
                let folder = row("launcher-folder-row", "Folder", held, palette)
                    .w(px(60.))
                    .flex_none();
                // Recipe needs the full address row. The held home leaves enough room beside Container host
                // for Name to share the next row, which still wraps on a narrow window.
                rows.child(
                    div()
                        .flex()
                        .flex_wrap()
                        .items_start()
                        .gap(px(ROW_GAP))
                        .children(container_host)
                        .child(folder)
                        .children(name.take()),
                )
                .into_any_element()
            } else {
                rows.children(container_host)
                    .child(row(
                        "launcher-repository-row",
                        "Repository",
                        self.repository_control(false, window, cx),
                        palette,
                    ))
                    .into_any_element()
            }
        } else if let Some(folder) = folder {
            let invalid = self.folder_problem(cx).is_some();
            let focused = !self.draft.holds(Field::Destination)
                && self.folder.focus_handle(cx).is_focused(window);
            let ink = if invalid {
                palette.warn
            } else if focused {
                palette.accent
            } else {
                palette.border
            };
            address_line(
                host,
                folder,
                "Folder",
                "launcher-folder-row",
                self.browse_button(palette, cx),
                ink,
                palette,
            )
            .into_any_element()
        } else {
            let ink = if self.checkout.repository.focus_handle(cx).is_focused(window) {
                palette.accent
            } else {
                palette.border
            };
            address_line(
                host,
                self.repository_control(true, window, cx),
                "Repository",
                "launcher-repository-row",
                None,
                ink,
                palette,
            )
            .into_any_element()
        };
        let address_and_name = div()
            .flex()
            .flex_wrap()
            .items_start()
            .gap(px(ROW_GAP))
            .child(
                div()
                    .flex_grow_1()
                    .flex_basis(px(ADDRESS_MIN_WIDTH))
                    .min_w(px(ADDRESS_MIN_WIDTH))
                    .max_w_full()
                    // Environment editors use the address's full width for recipe names and unavailable reasons.
                    .when(environment.is_some(), |fields| {
                        fields.flex_basis(gpui::relative(1.))
                    })
                    .child(address),
            )
            .children(name);
        // Under the host, as the old launcher said it there as well as beside Launch: the folder was taken on the
        // installation this host row named before.
        let installation_moved = self
            .draft
            .folder_installation_moved(&self.hosts(cx))
            .then(|| {
                choices::status_line(
                    "launcher-folder-installation-note",
                    FOLDER_INSTALLATION_MOVED.to_owned(),
                    palette,
                )
            });
        // The id makes the whole Destination block one target for the bridge, labels included, so a docs screenshot
        // can crop to it (the rows' own ids, from `row`, cover it only piecewise).
        let destination = div()
            .id("launcher-destination-section")
            .flex()
            .flex_col()
            .gap(px(ROW_GAP))
            .p(px(12.))
            .bg(theme::color(palette.card))
            .border_1()
            .border_color(theme::color(palette.border_dim))
            .rounded(px(6.))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .justify_between()
                    .items_center()
                    .gap(px(8.))
                    .child(
                        section_label("DESTINATION", palette)
                            .id("launcher-destination-heading")
                            .role(Role::Label)
                            .aria_label("Destination"),
                    )
                    .children(destination_switch),
            )
            .children(self.replacement_checkout_warning(palette, cx))
            .children(
                managed_checkout
                    .then(|| self.checkout_setup(palette, window, cx))
                    .flatten(),
            )
            .child(address_and_name)
            .children(managed_checkout.then(|| self.checkout_folder_line(palette, cx)))
            .children(managed_checkout.then(|| self.checkout_lifetime_note(palette, cx)))
            .children(
                self.host_picker
                    .note
                    .filter(|_| environment.is_none())
                    .map(|note| {
                        choices::status_line("launcher-host-note", note.to_owned(), palette)
                    }),
            )
            .children(installation_moved)
            .children(self.replacement_folder_hint(palette, cx))
            .children(environment.as_ref().and_then(|target| {
                let note = crate::environment_ui::ProviderUi(target.provider()).launch_notice()?;
                Some(
                    div()
                        .id("launcher-environment-notice")
                        .role(Role::Label)
                        .aria_label(note)
                        .text_size(metrics::TEXT_SM)
                        .text_color(theme::color(palette.fg_2))
                        .child(note),
                )
            }))
            .children(self.registration_note(palette))
            .children(self.folder_problem_line(palette, cx));
        let (on_tab, on_tab_enter) = (weak.clone(), weak.clone());
        let tabs = (!self.draft.holds(Field::LaunchKind)).then(|| {
            TabStrip::new(
                "launcher-tabs",
                "Launch kind",
                HoverHelp::tooltip("Launch an agent, a shell, or a custom command"),
            )
            .underlined()
            .tab(TabOption::new(
                "Agent",
                HoverHelp::tooltip(
                    "Start a supported agent with its launch choices, or a Bash or Zsh shell",
                ),
                Tab::Agent,
            ))
            .tab(TabOption::new(
                "Command",
                HoverHelp::tooltip("Command: run a command line you type, as written"),
                Tab::Command,
            ))
            .chosen(self.draft.tab)
            .on_choose(move |tab, window, cx| {
                let _ = on_tab.update(cx, |launcher, cx| {
                    launcher.switch_tab(*tab, window, cx);
                });
            })
            .on_enter(move |window, cx| {
                let _ = on_tab_enter.update(cx, |launcher, cx| launcher.press(window, cx));
            })
        });
        if self.draft.tab != Tab::Agent || self.draft.holds(Field::AgentType) {
            self.custom_chips_frame.clear();
        }
        let fields = match self.draft.tab {
            Tab::Agent => {
                let model = self.model_row(palette, window, cx);
                self.agent_fields(model, palette, window, cx)
            }
            Tab::Command => self.command_fields(palette, cx),
        };
        let choices = div()
            .flex()
            .flex_col()
            .gap(px(ROW_GAP))
            .children(tabs)
            .children(fields);
        let results_open = self.search_open(window, cx);
        let body = div()
            .flex()
            .flex_col()
            .gap(px(SECTION_GAP))
            .children(self.setup_source(palette))
            .child(destination)
            .child(choices)
            // After the choices, as the old listing was (`browse`).
            .children(self.browse_listing(palette, window, cx))
            .children(self.save_panel.clone());
        let wide = search::command_pane_fits(window);
        let body = if wide {
            let pane = self.command_pane(palette, window, cx);
            let scroll = scroll_handle("launcher-form-scroll".into(), window, cx);
            let form = div()
                .id("launcher-body")
                .debug_selector(|| "launcher-body".into())
                .flex_1()
                .min_w_0()
                .min_h_0()
                .py(metrics::DIALOG_PAD.0)
                .px(metrics::DIALOG_PAD.1)
                .overflow_y_scroll()
                .track_scroll(&scroll)
                .when(results_open, |form| form.opacity(0.55))
                .child(body);
            div()
                .flex()
                .size_full()
                .min_h_0()
                .child(pane)
                .child(crate::focus::scroll_region(
                    scroll_from_controls(form, &scroll),
                    &scroll,
                ))
                .into_any_element()
        } else {
            body.when(results_open, |form| form.opacity(0.55))
                .into_any_element()
        };

        // What a press has to say: the draft's own reason after a press it refused, or the helm's words refusing the
        // last launch. The helm's words are host text and are drawn escaped, as their own element.
        let lead = if restart {
            "Restart error."
        } else {
            "Not launched."
        };
        let alert =
            match (&refusal, &self.refusal_words(cx)) {
                (Some(reason), _) if self.pressed => Some(reason.append_to(
                    HostSentence::new("launcher-refusal-words").text(format!("{lead} ")),
                )),
                // The helm's timeout reply is not a refusal: the create may still happen, and its words say so.
                // Words complete as they stand are shown without the lead too: an environment create's failed run, which
                // the helm already words "Not launched. …", and the launcher's own words for an unknown outcome.
                (_, Some(words))
                    if self
                        .refused
                        .as_ref()
                        .is_some_and(|refused| refused.may_still_start || refused.complete) =>
                {
                    Some(HostSentence::new("launcher-refusal-words").host(words))
                }
                (_, Some(words)) => Some(
                    HostSentence::new("launcher-refusal-words")
                        .text(format!("{lead} "))
                        .host(words)
                        .text(self.refusal_suffix()),
                ),
                _ => None,
            };
        let alert = match &self.note {
            Some(Note::WakeRefusal(reason)) => {
                Some(reason.clone().prepend_text(format!("{lead} ")))
            }
            _ => alert,
        }
        .map(|sentence| refusal_alert("launcher-refusal", sentence, palette));
        // What a press of Launch has to say, in the old footer's order: the warning about what it stops of its source
        // first (Replace with's, or Restart with's while the agent works), then the YOLO question or the sandbox
        // question while one is up (a submit retires both before it can raise either), else the refusal. The warning
        // comes first so that a tall question, which scrolls in the notices region, cannot push it out of sight: the
        // question's answers launch with the warning's guard, so the warning must be on screen at that click. This
        // launcher's banner (a quick launch's needs-more-choices or number-key banner) comes right after the warning,
        // as its own element: it answers what the user did last, and the question or alert below it still shows. A
        // Template and command-field refusals are shown below that field (`view::search`).
        let warning = self.source_warning_line(palette, cx);
        let note = self.note_element(palette);
        let save_refusal = self.save_refusal_line(palette);
        let question = self
            .yolo_inline()
            .cloned()
            .map(IntoElement::into_any_element)
            .or_else(|| {
                self.sandbox_inline()
                    .cloned()
                    .map(IntoElement::into_any_element)
            })
            .or_else(|| {
                self.hook_inline(cx)
                    .cloned()
                    .map(IntoElement::into_any_element)
            });
        let notices = (warning.is_some()
            || note.is_some()
            || save_refusal.is_some()
            || question.is_some()
            || alert.is_some())
        .then(|| {
            let last = question.or(alert);
            div()
                .flex()
                .flex_col()
                .gap(px(6.))
                .children(warning)
                .children(note)
                .children(save_refusal)
                .children(last)
        });

        // Cancel and Close refuse while the YOLO question's host write or Save as template's save is out
        // (`Launcher::cancel`), and are drawn busy then.
        let closing_held = holding || self.template_saving(cx);
        let mut answers = vec![
            Button::new(
                "launcher-cancel",
                "Cancel",
                HoverHelp::tooltip("Cancel: close without starting anything"),
                Role::Button,
            )
            .tier(Tier::Neutral)
            .focus(&self.cancel_focus)
            .busy(closing_held)
            .on_click(on_launcher(weak.clone(), Launcher::cancel)),
        ];
        if self.draft.tab == Tab::Agent {
            answers.push(
                Button::new(
                    "launcher-reset",
                    "Reset choices",
                    HoverHelp::tooltip(if restart {
                        "Reset choices: restore the session's stored settings and clear the conversation selector"
                    } else {
                        "Reset choices: clear the agent, model and effort; remember permissions and the next agent's \
                         wrapper prefix"
                    }),
                    Role::Button,
                )
                .tier(Tier::Ghost)
                // Busy while the YOLO question's host write is out, as the old button was disabled while busy.
                .busy(holding)
                .on_click(on_launcher(weak.clone(), Launcher::reset_choices)),
            );
        }
        // Save as template, after Reset choices and before the main action (`save`).
        answers.extend(self.save_template_button(holding, cx));
        let verb = self.verb(cx);
        let key_ink = if refusal.is_some() || holding {
            palette.fg_2
        } else {
            palette.on_accent_dim
        };
        let help = match &refusal {
            Some(Refusal(words)) => HoverHelp::tooltip(words.clone()),
            None => HoverHelp::tooltip(match self.draft.mode {
                // The old stop-first wording while the agent works, so the help never contradicts "Stop and restart".
                Mode::RestartWith(_) if self.source_working(cx) => {
                    "Stop and restart: stop the working agent, then relaunch this session with these settings"
                }
                Mode::RestartWith(_) => "Restart: relaunch this session with these settings",
                Mode::ReplaceWith { .. } => {
                    "Replace: start this session, then delete the old one and its state"
                }
                Mode::New | Mode::Clone => "Launch: start the session",
            }),
        };
        answers.push(
            Button::new(
                "launcher-launch",
                self.action_name(&target, cx),
                help,
                Role::Button,
            )
            .content(return_key_content(verb, key_ink, palette))
            .tier(Tier::Primary)
            .refused(refusal.is_some())
            // Busy while the YOLO question's host write is out, when a press does nothing (`Launcher::press`).
            .busy(holding || self.hook_answer_pending(cx))
            .on_click(on_launcher(weak.clone(), move |launcher, window, cx| {
                if !launcher.accepts_default_folder(folder_action.as_ref(), cx) {
                    return;
                }
                // A click on this button can authorize only the source warning from its own rendered frame.
                if let Some(action) = &source_action {
                    if let Some(&source) = action.shown() {
                        launcher.press_at(Some(source), Mark::take(cx), window, cx);
                    }
                } else {
                    launcher.press_at(None, Mark::take(cx), window, cx);
                }
            })),
        );
        let title = search::command_title(&self.draft.mode);
        let (command, guidance, command_anchor) = self.command_header(palette, window, cx);
        // Across the top of the launcher, pinned while the body scrolls, as the old sticky banner was.
        let escalation = self.escalation_band(holding, palette, window, cx);
        let mut frame = DialogFrame::new("launcher", title, body, answers)
            .heading_control(command, guidance, Some(command_anchor))
            .width(px(WIDTH))
            .flat_margin_when_wide()
            .close(
                "Close session launcher",
                HoverHelp::tooltip("Close: discard this session setup"),
                closing_held,
                on_launcher(weak.clone(), Launcher::cancel),
            );
        if wide {
            // Two independent scroll viewports need the same finite space between the pinned header and actions.
            // Give the inline Save panel room in a tall window, and let the frame cap it to the current viewport.
            let height = if self.save_panel.is_some() {
                1000.
            } else {
                704.
            };
            frame = frame.fixed_height(px(height)).split_body();
        }
        if let Some(notices) = notices {
            // Under the actions row: the refusal, the Replace and Restart warnings and the YOLO question follow it.
            frame = frame.notices(notices).notices_below();
        }
        if let Some(escalation) = escalation {
            frame = frame.top_band(escalation);
        }
        frame
    }
}

impl Launcher {
    /// This launcher's banner ([`Note::Banner`]) as drawn at the top of the notices region, in the old
    /// `.partial-template-banner` look (a warning-colored edge on the left, the raised background and the ordinary ink,
    /// the words in bold). A template refusal note sits below the command field (`view::search`).
    fn note_element(&self, palette: &theme::Palette) -> Option<AnyElement> {
        let sentence = match self.note.as_ref()? {
            Note::Refusal(_) | Note::WakeRefusal(_) => return None,
            Note::Banner(sentence) => sentence,
        };
        Some(
            div()
                .id("launcher-banner")
                .debug_selector(|| "launcher-banner".into())
                .role(Role::Alert)
                .aria_label(sentence.accessible_name())
                .py(px(14.))
                .px(px(16.))
                .border_l(px(3.))
                .border_color(theme::color(palette.warn))
                .rounded(metrics::RADIUS_MD)
                .bg(theme::color(palette.bg_2))
                .text_color(theme::color(palette.fg_0))
                .font_weight(FontWeight::SEMIBOLD)
                .child(sentence.clone())
                .into_any_element(),
        )
    }
}

/// `.launch-composer-refusal-alert` under `id`, saying `sentence`: a bordered alert with a warning sign, in the warning
/// colors (red is kept for danger in the launcher). The press's reason and the helm's words use it.
fn refusal_alert(id: &'static str, sentence: HostSentence, palette: &theme::Palette) -> AnyElement {
    div()
        .id(id)
        .debug_selector(move || id.into())
        .role(Role::Alert)
        .aria_label(sentence.accessible_name())
        .flex()
        .items_start()
        .gap(px(8.))
        .py(px(8.))
        .px(px(10.))
        .border_1()
        .border_color(theme::color(palette.warn_border))
        .rounded(metrics::RADIUS_MD)
        .bg(theme::color(palette.warn_bg_dim))
        .text_size(metrics::TEXT_MD)
        .text_color(theme::color(palette.warn_bright))
        .child(div().flex_none().child("⚠"))
        .child(div().min_w_0().child(sentence))
        .into_any_element()
}

/// A registered host's name in the launcher (the footer, the host picker's button and list): its display name, with its phase in parentheses while it is not connected
/// (old `HostOption::label`). The phase is the app's own words; carried in host text, escaping them changes nothing.
pub(super) fn host_name_with_phase(host: &HostView) -> HostText {
    let name = display_name(host);
    if matches!(host.state, HostStateView::Connected { .. }) {
        name
    } else {
        HostText::from_host(format!("{} ({})", name.raw(), phase::words(&host.state)))
    }
}
impl Launcher {
    /// A visible press or ready quick-launch shortcut authorizes waking its existing destination. The draft's other
    /// refusal checks still run first so a missing argument or invalid destination cannot start the host.
    fn begin_host_wake(
        &mut self,
        source: SourceConsent,
        mark: Mark,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if launch::waking(cx) || self.refusal(cx).is_some() {
            return false;
        }
        let Some(host) = self
            .chosen_host(cx)
            .filter(|host| self.draft.can_wake(host))
            .cloned()
        else {
            return false;
        };
        if self.quick_launch_unseen
            && self.draft.folder_default_waiting()
            && self.settle_folder_default(cx)
        {
            self.sync_fields(window, cx);
        }
        if !launch::wake::start(cx.entity(), &self.draft, &host, source, mark, window, cx) {
            return false;
        }
        self.retire_checkout_root_refresh();
        // A visible press authorizes the folder shown at the time. Once the wake is sent,
        // a new connection's history must not replace that folder with a different default.
        if !self.quick_launch_unseen && self.draft.folder_default_waiting() {
            self.draft.settle_folder_default(
                Some(self.draft.folder.value().to_owned()),
                Install::of(&host),
            );
        }
        self.waiting_batches.clear();
        if let Some(layer) = self.layer.take() {
            layers::close(layer, window, cx);
        }
        true
    }

    /// A fresh connection has arrived. Use the regular folder/default/preview pipeline with the launch's original
    /// permission and focus mark. A hidden launcher that waits after this retains its wake's destination hold.
    pub(in crate::launcher) fn continue_after_wake(
        &mut self,
        source: SourceConsent,
        mark: Mark,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.quick_launch_unseen && self.draft.folder_default_waiting() {
            return self.start_or_wait_for_folder(mark, window, cx);
        }
        self.submit_or_wait_for_preview(source, mark, window, cx)
    }

    /// An unsuccessful preparation has sent no create or restart. Keep its reason for the mode's usual alert when
    /// restoring the form for the original press.
    pub(in crate::launcher) fn wake_refused(
        &mut self,
        reason: HostSentence,
        cx: &mut Context<Self>,
    ) {
        self.folder_wait = None;
        self.disarm_checkout_launch();
        self.note = Some(Note::WakeRefusal(reason));
        cx.notify();
    }
}
