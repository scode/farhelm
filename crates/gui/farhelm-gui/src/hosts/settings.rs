//! The host settings dialog: one host's connection, identity mark, checkout folder and permissions, opened from its
//! actions menu.
//!
//! SPEC.md, "Topology" (every host's settings dialog, its destination, alias, identity appearance and Checkouts
//! section), "Managed checkouts" (a host's own checkout folder) and "Agent-spawned sessions" (the Permissions section).
//! Ported from the old app's `hosts/settings_dialog.rs` (`HostSettingsDialog`, `HostDialogHeader`), its
//! `HostDestinationForm` and the settings handlers in `hosts.rs`, and the stylesheet's `.host-settings-*` rules, at
//! reference commit `24bc0580`. Here's the TLDR of its rules:
//!
//! - **The frame.** 480 pixels wide, a pinned header (the host's mark in its tile, its name, its phase and the build it
//!   reports, and the "Close host settings" control) and a pinned footer (the note that the checkout folder saves with
//!   Save while everything else saves as you make it, and Close), the sections scrolling between them in the old order:
//!   Connection, Appearance, Checkouts, Permissions.
//! - **Connection.** The SSH destination (SSH hosts only) and the alias, each a value with Edit. Edit swaps the value
//!   for a field over a [`DraftInput`] seeded with the stored raw value, with Save and Cancel; Save or Enter sends the
//!   field as typed, except that an alias of nothing but spaces goes as `null` (clearing it), as the old dialog sent
//!   it; nothing else is checked before sending (the helm trims an alias, clears it when empty, and refuses what it
//!   will not store). Success closes the field; a refusal keeps the draft with the helm's words under it, including the
//!   helm's answer that the change waits for a setup or Update running on the host. Escape closes an open field rather
//!   than the dialog, and does nothing while its save is in flight.
//!   SSH hosts also have a Tailscale switch, which saves on its own route and reconnects the host on success.
//! - **During a launch.** Destination and Tailscale edits wait while a session launch is in flight.
//!   A destination's Save sends nothing while a session launch is in flight
//!   ([`super::launch_in_flight`]), and its row says "Destination: not started, a session launch is in progress;
//!   try again when it finishes", the draft kept as typed. The Tailscale switch keeps the pushed value in that case.
//! - **Appearance.** The sixteen icons and the eight colors, each a pressed-or-not choice ([`appearance`]), a click
//!   saving icon and color together at once, and a preview of the mark as the session list draws it.
//! - **Checkouts.** The host's own checkout folder, on every host kind: the shared explicitly saved folder field
//!   ([`CheckoutField::folder`] over [`FolderScope::Host`]), made once per opening, which reads
//!   `/api/hosts/{id}/checkout-root` as the dialog opens, shows the inherited default as its placeholder, and offers
//!   Use default while the host has a folder of its own. Its draft, refusal and saved line are the field's own; the
//!   dialog only draws it, under the old dialog's note.
//! - **Permissions.** Five switches, each saving its own stored value at once on its own route, with help that
//!   explains the current behavior.
//! - **Saving.** Everything saves through the operation store under the host's key ([`OpKey::host`]), the checkout
//!   folder included, which admits one request at a time: a press while another save on this host is in flight sends
//!   nothing, whichever control it is on, and a line beside that control says so ([`Refused`]). A switch or a choice
//!   shows the value being saved only while its save is in flight and the stored value otherwise, so a refused save
//!   shows the stored value again, with the helm's words under the control until its next save starts (SPEC.md: "A
//!   failed save restores the last known value and shows an adjacent error"). The refusal also shows under the host's
//!   row in the Hosts panel, as every refused work on a host does.
//! - **Opening and closing.** [`open_settings`] is the entry point. Focus starts on the first Edit (the destination's,
//!   or the alias's on a host without one), as the old dialog's did. Escape, the header's close control and Close close
//!   it, and focus returns to the control it was opened from (the menu's toggle); a press outside does nothing. It closes
//!   by itself when the registry no longer lists the host. A save in flight goes on after it closed.
//!
//! NOTE: The old dialog disabled every control while any host write ran and while a field was open, the checkout folder
//! included (its `disabled: busy || editing.is_some()`). That lock is not ported, the folder's included: here only the
//! other Connection field's Edit waits for an open field, and every other control stays live. The operation store's
//! one request per key already keeps two saves on this host from running at once, so a lock would only add a second
//! way of saying the same thing. And the old lock's other reason, that a toggle's outcome would replace an open field's
//! refusal in the one outcome line the dialog had, does not hold here, where every control keeps its own refusal. A
//! press refused for the key says so beside its control instead of the control being drawn unavailable; the parity
//! file's deviations record it.

mod appearance;
#[cfg(test)]
mod tests;

use std::rc::Rc;

use farhelm_helm_client::{ApiPath, Method, PathError};
use farhelm_proto::host_appearance::{HostColor, HostIcon};
use farhelm_proto::ui_channel::{Availability, Collection, HostView};
use gpui::{
    AnyElement, AnyWindowHandle, App, AppContext as _, Context, Div, Entity, FocusHandle,
    FontWeight, InteractiveElement as _, IntoElement, ParentElement as _, Render, SharedString,
    StatefulInteractiveElement as _, Styled as _, Subscription, WeakEntity, Window, div,
    prelude::FluentBuilder as _, px,
};

use super::identity::{display_name, host_mark, phase};
use super::offers::HostKind;
use super::phase_dot_and_words;
use crate::assets::TERMINAL_FONT;
use crate::components::{
    Button, Checkbox, DialogFrame, HoverHelp, Role, TextInput, TextInputState, Tier,
};
use crate::drafts::{self, Draft, DraftInput};
use crate::focus::{self, Mark, ReturnTo};
use crate::host_text::{self, HostSentence, HostText, Line};
use crate::layers::{self, Dialog, DismissHook, Dismissal, LayerId, Subject};
use crate::operations::{Command, OpKey, Outcome, Work};
use crate::session;
use crate::settings::{CheckoutField, FolderScope, Refused};
use crate::store::HelmStore;
use crate::theme::{self, Palette, metrics};

/// `.host-settings-dialog`'s width.
const WIDTH: f32 = 480.;
/// The gap between sections, and inside one between its title and its settings (`.host-settings-section`).
const SECTION_GAP: f32 = 14.;
/// A later section's room above its title, under the rule that separates it from the one before.
const SECTION_RULE_PAD: f32 = 20.;
/// `.host-settings-section-title`'s size.
const SECTION_TITLE: f32 = 11.;
/// The settings' labels and values: the browser's default 16 pixels the old rows inherited, on the line Settings uses
/// for the same labels.
const LABEL_TEXT: f32 = 16.;
const LABEL_LINE_HEIGHT: f32 = 19.4;
/// `.host-settings-help`: 12 pixels on the `normal` line of the bundled face.
const HELP_LINE_HEIGHT: f32 = 15.;
/// `.host-settings-row`'s label column (`6.5rem`) and its gaps.
const ROW_LABEL: f32 = 104.;
const ROW_GAP: f32 = 12.;
/// `.setting-group`'s gap: a description sits this close under its own label.
const GROUP_GAP: f32 = 4.;
const TAILSCALE_SWITCH: &str = "host-settings-tailscale";
/// The host's mark in the header's tile.
const TILE_MARK: f32 = 24.;

/// The footer's note, the old dialog's words: the checkout folder is the one setting here that waits for its Save.
const FOOTER_NOTE: &str = "Checkout folder saves with Save; other changes save as you make them";
/// The note under the checkout folder field, the old dialog's words. The field's own words (its label, hover help, Save
/// and Use default) are the shared field's ([`CheckoutField`]).
const FOLDER_NOTE: &str = "An empty field uses the default from Settings → Checkouts. ~ means this host's home folder. \
                           Changes apply to new checkouts; existing checkouts stay where they are.";
/// The close controls' shared hover help, the old dialog's.
const CLOSE_HELP: &str = "Close: done with this host's settings";
/// The appearance picker's name for [`Refused`], the stem of its line ids (`-error`, `-busy`).
const APPEARANCE: &str = "host-settings-appearance";

/// Open host `host`'s settings dialog on top of everything else in `window`: the entry point the host menu's Settings
/// calls. `opener` is the control the user opened it from (the menu's toggle), which closing returns focus to on every
/// platform ([`ReturnTo::Control`]), also where a click leaves a button unfocused (macOS), as the old dialog returned
/// it to the row's ⋯ toggle. `mark` is the [`Mark`] taken in the handler of the user's action.
///
/// Returns the dialog's layer, or `None` when nothing opened: the window is inert, the registry does not list the
/// host, or the user moved on since `mark`.
pub fn open_settings(
    host: i64,
    opener: &FocusHandle,
    mark: &Mark,
    window: &mut Window,
    cx: &mut App,
) -> Option<LayerId> {
    let store = session::current_store(cx)?;
    let key = HostView::key(host);
    let view = store.read(cx).collections().get::<HostView>(&key)?.clone();
    let dialog = cx.new(|cx| HostSettingsDialog::new(&view, &store, window, cx));
    let start = {
        let dialog = dialog.read(cx);
        let first = if HostKind::of(&view).edits_destination() {
            Field::Destination
        } else {
            Field::Alias
        };
        dialog.edit_focus[first.index()].clone()
    };
    let weak = dialog.downgrade();
    let on_dismiss: DismissHook = Rc::new(move |window, cx| dismissed(&weak, window, cx));
    let layer = layers::open_dialog(
        Dialog {
            view: dialog.clone().into(),
            container: cx.focus_handle(),
            start,
            subject: Some(Subject::when(store, move |store, _| gone(store, &key))),
            // None: Enter on the dialog itself does nothing, as no one action is the dialog's.
            main_action: None,
            // SPEC.md, "Opening and closing", lists the dialogs that share the host and Settings installer among those
            // that keep focus on the control in use.
            keeps_focus_on_empty_click: true,
            returns_to: ReturnTo::Control(opener.clone()),
            on_dismiss: Some(on_dismiss),
            placement: layers::DialogPlacement::Centered,
            dismiss_on_outside_press: false,
        },
        mark,
        window,
        cx,
    )?;
    dialog.update(cx, |dialog, _| dialog.layer = Some(layer));
    #[cfg(test)]
    layers::record_opened(&dialog, cx);
    Some(layer)
}

/// Start the same route edit the host settings switch sends so the golden can show a server refusal.
#[cfg(test)]
pub(crate) fn enable_tailscale_for_golden(cx: &mut App) {
    let dialog = layers::last_opened::<HostSettingsDialog>(cx).expect("host settings opened");
    dialog.update(cx, |dialog, cx| dialog.toggle_tailscale(true, cx));
}

/// Whether the registry proves host `key` gone: a hosts list read whole that no longer lists it. A failed refresh keeps
/// the hosts it had and proves nothing.
pub(super) fn gone(store: &HelmStore, key: &str) -> bool {
    let collections = store.collections();
    collections
        .meta(Collection::Hosts)
        .is_some_and(|meta| meta.availability == Availability::Available)
        && collections.get::<HostView>(key).is_none()
}

/// Escape on the dialog: close an open field rather than the dialog, and nothing at all while that field's save is in
/// flight (the old dialog's Escape followed its field's Cancel). With no field open, the dialog closes.
fn dismissed(
    dialog: &WeakEntity<HostSettingsDialog>,
    window: &mut Window,
    cx: &mut App,
) -> Dismissal {
    let Some(dialog) = dialog.upgrade() else {
        return Dismissal::Close;
    };
    let (editing, saving) = {
        let dialog = dialog.read(cx);
        let saving = dialog
            .editing
            .is_some_and(|field| dialog.input(field).draft().is_saving());
        (dialog.editing, saving)
    };
    match editing {
        None => Dismissal::Close,
        Some(_) if saving => Dismissal::Stay,
        Some(_) => {
            let mark = Mark::take(cx);
            dialog.update(cx, |dialog, cx| dialog.leave_editor(&mark, window, cx));
            Dismissal::Stay
        }
    }
}

// ---- Connection: the destination and the alias ----------------------------------------------------------------------

/// One of the Connection section's text settings, each edited the same way (see the module docs).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Field {
    Destination,
    Alias,
}

impl Field {
    /// Its slot in the dialog's per-field state: its place in the declaration.
    fn index(self) -> usize {
        self as usize
    }

    /// Its stable key, which its controls' element ids are built from.
    fn key(self) -> &'static str {
        match self {
            Field::Destination => "destination",
            Field::Alias => "alias",
        }
    }

    /// Whether a save of it waits for a session launch in flight ([`super::launch_in_flight`]), and so the words
    /// its refusal starts with. Only the destination does: it is where a launch on the host dials, so changing it under
    /// a launch is the race the old app's one page-wide token kept out. The migration's orchestrator decided on
    /// 2026-11-14 that the alias and the rest of the dialog's saves go ahead during a launch, where the old app made
    /// all of them wait.
    fn waits_for_launches(self) -> Option<&'static str> {
        match self {
            Field::Destination => Some("Destination: "),
            Field::Alias => None,
        }
    }

    /// Its name for [`Refused`], `host-settings-<key>`, the stem of its row's line ids (`-error`, `-busy`).
    fn control(self) -> &'static str {
        match self {
            Field::Destination => "host-settings-destination",
            Field::Alias => "host-settings-alias",
        }
    }

    /// The stored raw value the field starts from. An unset value opens as empty.
    fn stored(self, view: &HostView) -> Option<&str> {
        match self {
            Field::Destination => view.destination.as_deref(),
            Field::Alias => view.alias.as_deref(),
        }
    }

    /// `POST /api/hosts/{id}/destination` with `{"ssh": value}`, or `POST /api/hosts/{id}/alias` with `{"alias":
    /// value}`, the value as typed. An alias with nothing but spaces is sent as `null`, which clears it, as the old
    /// dialog sent it; the helm would clear it either way.
    fn command(self, host: i64, value: &str) -> Result<Command, PathError> {
        let key = HostView::key(host);
        let (route, body) = match self {
            Field::Destination => ("destination", serde_json::json!({ "ssh": value })),
            Field::Alias if value.trim().is_empty() => {
                ("alias", serde_json::json!({ "alias": null }))
            }
            Field::Alias => ("alias", serde_json::json!({ "alias": value })),
        };
        Ok(Command::new(
            Work::Save,
            Method::Post,
            ApiPath::api(&["hosts", &key, route])?,
            Some(body),
            format!("POST /api/hosts/[host]/{route}"),
        ))
    }
}

// ---- Permissions -----------------------------------------------------------------------------------------------------

/// The two browser permissions' help, which is also their hover help and does not change with the switch.
const LINKS_HELP: &str = "Agents on this host may open public web addresses. Local-network addresses still need their own permission.";
const LOCAL_LINKS_HELP: &str = "Agents on this host may make your browser load addresses on your own machine or \
                                private network without an approval question.";

/// The command that sets host `host` to start YOLO sessions without asking: exactly what this dialog's "Start YOLO
/// sessions without asking" switch sends when turned on. The launcher's YOLO question sends it for its "don't ask again"
/// answer (SPEC.md, "Creation": it "first sets the host to start YOLO sessions without asking, exactly as its settings
/// would"), so the two can never send different requests. It is [`Work::Save`] on the host's own key, as the switch's is.
pub(crate) fn start_yolo_without_asking_command(host: i64) -> Result<Command, PathError> {
    Permission::Yolo.command(host, true)
}

/// One of the Permissions section's switches, each its own stored value on its own route.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Permission {
    Yolo,
    OwnSessions,
    Commands,
    Links,
    LocalLinks,
}

impl Permission {
    /// Every switch, in the order the section draws them.
    const ALL: [Permission; 5] = [
        Permission::Yolo,
        Permission::OwnSessions,
        Permission::Commands,
        Permission::Links,
        Permission::LocalLinks,
    ];

    /// Its slot in the dialog's switches: its place in the declaration, which is [`Permission::ALL`]'s order.
    fn index(self) -> usize {
        self as usize
    }

    /// The route's last segment, which is also the switch's element id after `host-`.
    fn route(self) -> &'static str {
        match self {
            Permission::Yolo => "yolo-without-asking",
            Permission::OwnSessions => "own-sessions-without-asking",
            Permission::Commands => "commands-without-asking",
            Permission::Links => "links-without-asking",
            Permission::LocalLinks => "local-links-without-asking",
        }
    }

    /// The switch's element id (`host-<route>`), which tests and the test bridge find it by; its failure line's is this
    /// with `-error`.
    fn id(self) -> String {
        format!("host-{}", self.route())
    }

    /// The value the host stores for it.
    fn stored(self, view: &HostView) -> bool {
        match self {
            Permission::Yolo => view.yolo_without_asking,
            Permission::OwnSessions => view.own_sessions_without_asking,
            Permission::Commands => view.commands_without_asking,
            Permission::Links => view.links_without_asking,
            Permission::LocalLinks => view.local_links_without_asking,
        }
    }

    /// `POST /api/hosts/{id}/<route>` with the new value, required, under the field its route reads: the setting's own
    /// name for the three agent grants, `allowed` for the two browser permissions.
    fn command(self, host: i64, on: bool) -> Result<Command, PathError> {
        let field = match self {
            Permission::Yolo => "yolo_without_asking",
            Permission::OwnSessions => "own_sessions_without_asking",
            Permission::Commands => "commands_without_asking",
            Permission::Links | Permission::LocalLinks => "allowed",
        };
        let mut body = serde_json::Map::new();
        body.insert(field.to_owned(), serde_json::Value::Bool(on));
        Ok(Command::new(
            Work::Save,
            Method::Post,
            ApiPath::api(&["hosts", &HostView::key(host), self.route()])?,
            Some(serde_json::Value::Object(body)),
            format!("POST /api/hosts/[host]/{}", self.route()),
        ))
    }

    fn label(self) -> &'static str {
        match self {
            Permission::Yolo => "Start YOLO sessions without asking",
            Permission::OwnSessions => "Manage sessions on this host without asking",
            Permission::Commands => "Run farhelm commands without asking",
            Permission::Links => "Open links without asking",
            Permission::LocalLinks => "Open local-network links without asking",
        }
    }

    /// The hover help, the old row's `data-tooltip`.
    fn tooltip(self) -> &'static str {
        match self {
            Permission::Yolo => "Start YOLO sessions on this host without asking first",
            Permission::OwnSessions => {
                "Let agents in this host's sessions manage sessions on this same host without asking first"
            }
            Permission::Commands => {
                "Let agents in this host's sessions start, stop, change and delete sessions and templates without \
                 asking first"
            }
            Permission::Links => LINKS_HELP,
            Permission::LocalLinks => LOCAL_LINKS_HELP,
        }
    }

    /// The help under the switch, explaining the behavior the shown values give (`shown` reads any switch's): the old
    /// dialog's `yolo_help`, `own_sessions_help` and `commands_help`. The two agent-command grants overlap, so each one's
    /// help also says what the other one shown covers.
    fn help(self, shown: impl Fn(Permission) -> bool) -> String {
        let on = shown(self);
        match self {
            Permission::Yolo => {
                let tail = if on {
                    "These sessions start on this host without asking you to confirm."
                } else {
                    "Farhelm asks you to confirm each of these launches on this host."
                };
                format!(
                    "YOLO bypasses an agent's safety checks. Farhelm also treats custom flags and saved agent launches \
                     with no recorded permission as YOLO. {tail}"
                )
            }
            Permission::OwnSessions => {
                let choice = if on {
                    "Agents in this host's sessions start, stop, restart, rename, clone and delete sessions on this \
                     same host without asking you. Clones must also come from this host."
                } else {
                    "This setting does not let agents in this host's sessions start, stop, restart, rename, clone or \
                     delete sessions on this same host without asking you."
                };
                let overlap = if shown(Permission::Commands) {
                    "Run farhelm commands without asking already covers these session actions; this switch keeps its \
                     own saved value."
                } else {
                    "Run farhelm commands without asking also includes these session actions."
                };
                format!(
                    "{choice} Requests for other hosts and template changes still ask unless Run farhelm commands \
                     without asking covers them. Host changes always need your approval for each request. {overlap}"
                )
            }
            Permission::Commands if on => "Agents in this host's sessions start, stop, restart, rename, clone and \
                                           delete sessions on any host, and edit and delete launch templates, \
                                           without asking you. Changes to hosts always need your approval for each \
                                           request."
                .to_owned(),
            Permission::Commands if shown(Permission::OwnSessions) => {
                "Agents in this host's sessions can manage sessions on this same host under Manage sessions on this \
                 host without asking. Requests for other hosts, clones from other hosts, and edits or deletes of \
                 launch templates still ask. Changes to hosts always need your approval for each request."
                    .to_owned()
            }
            Permission::Commands => "When an agent in one of this host's sessions asks Farhelm to start, stop, \
                                     restart, rename, clone or delete a session, or to edit or delete a launch \
                                     template, Farhelm asks you first. Changes to hosts always need your approval \
                                     for each request."
                .to_owned(),
            Permission::Links => LINKS_HELP.to_owned(),
            Permission::LocalLinks => LOCAL_LINKS_HELP.to_owned(),
        }
    }
}

/// One switch's state in the open dialog: its draft, which holds the value being saved and the last refusal, and its
/// focus handle.
struct Switch {
    draft: Draft<bool>,
    focus: FocusHandle,
}

/// `POST /api/hosts/{id}/appearance` with both words, as the helm requires them together.
fn appearance_command(host: i64, choice: (HostIcon, HostColor)) -> Result<Command, PathError> {
    Ok(Command::new(
        Work::Save,
        Method::Post,
        ApiPath::api(&["hosts", &HostView::key(host), "appearance"])?,
        Some(serde_json::json!({ "icon": choice.0, "color": choice.1 })),
        "POST /api/hosts/[host]/appearance",
    ))
}

// ---- The dialog ------------------------------------------------------------------------------------------------------

/// The open dialog: which host it edits, each setting's draft and focus handle, and its layer.
pub(crate) struct HostSettingsDialog {
    /// The host's registry id. Fixed for the dialog's life.
    host: i64,
    /// The field whose editor is open, if any.
    editing: Option<Field>,
    /// The destination's and the alias's fields, each made afresh from the stored value when its Edit opens it.
    destination: DraftInput,
    alias: DraftInput,
    /// Each field's Edit button, by [`Field::index`]: where focus starts, and where it goes back when the field closes.
    edit_focus: [FocusHandle; 2],
    /// The open field's Save and Cancel.
    save_focus: FocusHandle,
    cancel_focus: FocusHandle,
    /// The icon and color the last appearance click chose, while its save is in flight.
    appearance: Draft<(HostIcon, HostColor)>,
    /// The route switch saves on its own verb and follows the pushed value after the save settles.
    tailscale: Switch,
    /// Route edits wait until a launch has finished using the current connection.
    tailscale_launch_refused: bool,
    /// One per [`Permission`], by [`Permission::index`].
    switches: Vec<Switch>,
    /// The Checkouts section's folder field, a view of its own made with the dialog, so its read of the host's folder
    /// is sent as the dialog opens and its answer reaches only this opening.
    checkout: Entity<CheckoutField>,
    /// The control here whose press sent nothing last because another save held the host's key (a field's Save, the
    /// picker, a permission). The folder field, a view of its own, keeps its own.
    refused: Refused,
    /// The field whose last Save sent nothing because a session launch was in flight ([`Field::waits_for_launches`]):
    /// its row says so where its save errors show, until its next press gets past the launch check or it closes, and it
    /// replaces the busy line of an earlier press. The draft stays as typed.
    launch_refused: Option<Field>,
    /// Set right after the layer opens; what the close controls close.
    layer: Option<LayerId>,
    /// The store's pushes, which an open field follows while untouched.
    _pushes: Subscription,
}

impl HostSettingsDialog {
    /// A dialog over `view`, following `store` while it lives, and sending the checkout folder's read now. The store is
    /// the current UI session's, which the folder field's read needs too ([`CheckoutField`]: without a session it sends
    /// nothing and stays read-only); a reset closes every dialog, so the dialog never outlives it.
    fn new(
        view: &HostView,
        store: &Entity<HelmStore>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let pushes = cx.observe_in(store, window, |dialog, store, window, cx| {
            dialog.pushed(&store, window, cx)
        });
        HostSettingsDialog {
            host: view.id,
            editing: None,
            destination: Self::field_input(Field::Destination, view, window, cx),
            alias: Self::field_input(Field::Alias, view, window, cx),
            edit_focus: [
                cx.focus_handle().tab_stop(true),
                cx.focus_handle().tab_stop(true),
            ],
            save_focus: cx.focus_handle().tab_stop(true),
            cancel_focus: cx.focus_handle().tab_stop(true),
            appearance: Draft::load((view.icon, view.color)),
            tailscale: Switch {
                draft: Draft::load(view.tailscale),
                focus: cx.focus_handle().tab_stop(true),
            },
            tailscale_launch_refused: false,
            switches: Permission::ALL
                .iter()
                .map(|_| Switch {
                    // Shown only while being saved, and set by the click that saves it (`shown`).
                    draft: Draft::load(false),
                    focus: cx.focus_handle().tab_stop(true),
                })
                .collect(),
            checkout: cx.new(|cx| {
                CheckoutField::folder(
                    FolderScope::Host(HostView::key(view.id)),
                    FOLDER_NOTE,
                    window,
                    cx,
                )
            }),
            layer: None,
            refused: Refused::default(),
            launch_refused: None,
            _pushes: pushes,
        }
    }

    /// `field`'s text field over a fresh draft of its stored value in `view`, Enter saving it. Enter saves whichever
    /// field is open, which is this one: only the open field is drawn as a text input.
    fn field_input(
        field: Field,
        view: &HostView,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> DraftInput {
        DraftInput::new(
            field.stored(view).unwrap_or_default(),
            TextInputState::new(window, cx),
            Self::input_of(field),
            |dialog, window, cx| {
                if let Some(open) = dialog.editing {
                    dialog.save_field(open, window, cx);
                }
            },
            window,
            cx,
        )
    }

    /// `field`'s text field.
    fn input(&self, field: Field) -> &DraftInput {
        match field {
            Field::Destination => &self.destination,
            Field::Alias => &self.alias,
        }
    }

    /// How a save reaches `field`'s text field inside the dialog.
    fn input_of(field: Field) -> fn(&mut Self) -> &mut DraftInput {
        match field {
            Field::Destination => |dialog| &mut dialog.destination,
            Field::Alias => |dialog| &mut dialog.alias,
        }
    }

    /// The host as the store holds it now; `None` once the registry stopped listing it, or with the window inert.
    fn view(&self, cx: &App) -> Option<HostView> {
        let store = session::current_store(cx)?;
        store
            .read(cx)
            .collections()
            .get::<HostView>(&HostView::key(self.host))
            .cloned()
    }

    /// The store changed: an open field follows its stored value while the user has not touched it.
    fn pushed(&mut self, store: &Entity<HelmStore>, window: &mut Window, cx: &mut Context<Self>) {
        let Some(field) = self.editing else {
            return;
        };
        let stored = store
            .read(cx)
            .collections()
            .get::<HostView>(&HostView::key(self.host))
            .map(|view| field.stored(view).unwrap_or_default().to_owned());
        if let Some(stored) = stored {
            Self::input_of(field)(self).follow(&stored, window, cx);
        }
    }

    /// Edit `field`: put its field, seeded with the stored value, in the value's place, and move focus into it, as the
    /// old field's autofocus took it, on every platform. Does nothing while a field is open. Called from Edit's click
    /// (or Enter or Space on it), whose action the [`Mark`] taken here stands for.
    fn edit(&mut self, field: Field, window: &mut Window, cx: &mut Context<Self>) {
        if self.editing.is_some() {
            return;
        }
        let Some(view) = self.view(cx) else {
            return;
        };
        *Self::input_of(field)(self) = Self::field_input(field, &view, window, cx);
        self.editing = Some(field);
        // The field is drawn only from the next frame on; the coordinator makes the move once it is, unless the user
        // moved on first.
        let mark = Mark::take(cx);
        let input = self.input(field).input().focus_handle(cx);
        focus::focus_in_layer(&input, &mark, window, cx);
        cx.notify();
    }

    /// Close the open field, discarding its draft, and move focus back to its Edit, which takes the field's place
    /// again, for the action `mark` stands for (Cancel, Escape, or the Save whose success this is). Nothing moves when
    /// the user has moved on since `mark` (a click elsewhere while a save was in flight, say).
    fn leave_editor(&mut self, mark: &Mark, window: &mut Window, cx: &mut Context<Self>) {
        let Some(field) = self.editing.take() else {
            return;
        };
        self.launch_refused = None;
        focus::focus_in_layer(&self.edit_focus[field.index()], mark, window, cx);
        cx.notify();
    }

    /// Cancel: close the open field. Refused while its save is in flight.
    fn cancel(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let saving = self
            .editing
            .is_some_and(|field| self.input(field).draft().is_saving());
        if !saving {
            let mark = Mark::take(cx);
            self.leave_editor(&mark, window, cx);
        }
    }

    /// Save the open `field` as typed. Success closes it and a refusal stays on the row. An untouched alias closes
    /// like Cancel because sending its old value could undo a rename still waiting for the host's locks.
    fn save_field(&mut self, field: Field, window: &mut Window, cx: &mut Context<Self>) {
        if self.editing != Some(field) || self.input(field).draft().is_saving() {
            return;
        }
        if field == Field::Alias && !self.input(field).draft().is_touched() {
            self.cancel(window, cx);
            return;
        }
        // Each press leaves only its own line: a launch refusal replaces a busy line from an earlier press, and a press
        // that gets past the launch check takes the launch line away, whatever then becomes of its save.
        if field.waits_for_launches().is_some() && super::launch_in_flight(cx) {
            self.launch_refused = Some(field);
            self.refused.record(field.control(), &Ok(()));
            cx.notify();
            return;
        }
        self.launch_refused = None;
        let (host, mark) = (self.host, Mark::take(cx));
        let handle = window.window_handle();
        let started = DraftInput::save(
            self,
            Self::input_of(field),
            &OpKey::host(&HostView::key(host)),
            |value| field.command(host, value),
            window,
            cx,
            move |_, settled, cx| {
                if let Outcome::Done(_) = settled.outcome {
                    leave_editor_later(mark, handle, cx);
                }
            },
        );
        // Other work on the host may be in flight, which the row then says; the draft is untouched.
        self.refused.record(field.control(), &started);
        if let Err(not_started) = started {
            tracing::debug!(?field, ?not_started, "host setting not saved");
        }
        cx.notify();
    }

    /// Save `value` into the draft `field` reaches at once, with `command`, under the host's key: what a switch or a
    /// picker choice does on a click. A press while that draft's own save is in flight, or while other work on the host
    /// is (the operation store refuses the key), sends nothing: the draft, not saving, goes on showing the stored value,
    /// and `control`, the element id the press was on, says why ([`Refused`]).
    fn save_now<T: Clone + PartialEq + std::fmt::Debug + 'static>(
        &mut self,
        control: impl Into<SharedString>,
        field: impl Fn(&mut Self) -> &mut Draft<T> + Copy + 'static,
        value: T,
        command: impl FnOnce(&T) -> Result<Command, PathError>,
        cx: &mut Context<Self>,
    ) {
        if field(self).edit(value) {
            let key = OpKey::host(&HostView::key(self.host));
            let started = drafts::save(self, field, &key, command, cx, |_, _, _| {});
            self.refused.record(control, &started);
            if let Err(not_started) = started {
                tracing::debug!(value = ?field(self).value(), ?not_started, "host setting not saved");
            }
        } else {
            // The draft's own save is in flight: work on the same key.
            self.refused.refuse(control);
        }
        cx.notify();
    }

    /// The user turned `permission`'s switch to `on`: save it on its route.
    fn toggle(&mut self, permission: Permission, on: bool, cx: &mut Context<Self>) {
        let (index, host) = (permission.index(), self.host);
        self.save_now(
            permission.id(),
            move |dialog: &mut Self| &mut dialog.switches[index].draft,
            on,
            |on| permission.command(host, *on),
            cx,
        );
    }

    /// Save a route change through the same host operation key as a destination edit.
    fn toggle_tailscale(&mut self, on: bool, cx: &mut Context<Self>) {
        self.tailscale_launch_refused = super::launch_in_flight(cx);
        if self.tailscale_launch_refused {
            self.refused.record(TAILSCALE_SWITCH, &Ok(()));
            cx.notify();
            return;
        }
        let host = HostView::key(self.host);
        self.save_now(
            TAILSCALE_SWITCH,
            |dialog: &mut Self| &mut dialog.tailscale.draft,
            on,
            |on| {
                Ok(Command::new(
                    Work::Save,
                    Method::Post,
                    ApiPath::api(&["hosts", &host, "tailscale"])?,
                    Some(serde_json::json!({ "tailscale": on })),
                    "POST /api/hosts/[host]/tailscale",
                ))
            },
            cx,
        );
    }

    /// The user chose `choice` (a new icon with the shown color, or the shown icon with a new color): save the pair.
    fn choose(&mut self, choice: (HostIcon, HostColor), cx: &mut Context<Self>) {
        let host = self.host;
        self.save_now(
            APPEARANCE,
            |dialog: &mut Self| &mut dialog.appearance,
            choice,
            |choice| appearance_command(host, *choice),
            cx,
        );
    }

    /// Close the dialog, from either close control. A close, not a dismissal: an open field does not hold it, and a
    /// save in flight goes on in the operation store.
    fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(layer) = self.layer.take() {
            layers::close(layer, window, cx);
        }
    }
}

/// What a save-at-once control shows: the value being saved while `draft`'s save is in flight, `stored` otherwise. With
/// no optimistic copy, a refused save therefore shows the stored value again with nothing to restore.
fn shown<T: Clone + PartialEq>(draft: &Draft<T>, stored: T) -> T {
    if draft.is_saving() {
        draft.value().clone()
    } else {
        stored
    }
}

/// Close the open field of the dialog `cx` belongs to, from a save's completion, which has no window at hand: once the
/// completion has returned, in `window` if it is still open. Deferred only to leave the dialog's update and reach its
/// window; the move itself waits for no frame. `mark` is the Save's, so focus goes back to Edit only if the user has
/// not moved on since, and any key press counts, an Escape the open field refused while the save ran included: the
/// user did press a key, and focus then stays with the coordinator's recovery.
fn leave_editor_later(mark: Mark, window: AnyWindowHandle, cx: &mut Context<HostSettingsDialog>) {
    let dialog = cx.weak_entity();
    cx.defer(move |cx| {
        let _ = window.update(cx, |_, window, cx| {
            let _ = dialog.update(cx, |dialog, cx| dialog.leave_editor(&mark, window, cx));
        });
    });
}

/// A handler that runs `act` on the dialog, if it is still open.
fn on_dialog<E>(
    dialog: WeakEntity<HostSettingsDialog>,
    act: fn(&mut HostSettingsDialog, &mut Window, &mut Context<HostSettingsDialog>),
) -> impl Fn(&E, &mut Window, &mut App) + 'static {
    move |_, window, cx| {
        let _ = dialog.update(cx, |dialog, cx| act(dialog, window, cx));
    }
}

// ---- Drawing ---------------------------------------------------------------------------------------------------------

impl Render for HostSettingsDialog {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = theme::palette(cx);
        let weak = cx.weak_entity();
        let close = Button::new(
            "host-settings-footer-close",
            "Close",
            HoverHelp::tooltip(CLOSE_HELP),
            Role::Button,
        )
        .tier(Tier::Neutral)
        .on_click(on_dialog(weak.clone(), HostSettingsDialog::close));
        // Gone from the registry: the dialog closes as its subject goes, and until then shows nothing to change.
        let (title, heading, tile, body): (Line, _, _, _) = match self.view(cx) {
            Some(view) => {
                let kind = HostKind::of(&view);
                let mut sections = vec![self.connection(&view, kind, palette, cx)];
                if kind.supports_appearance() {
                    sections.push(self.appearance_section(&view, palette, cx));
                }
                // On every host kind, unlike Appearance (SPEC.md, "Topology").
                sections.push(section(
                    "checkouts",
                    "Checkouts",
                    false,
                    vec![self.checkout.clone().into_any_element()],
                    palette,
                ));
                sections.push(self.permissions(&view, palette, cx));
                let tile = kind
                    .supports_appearance()
                    .then(|| host_mark(view.icon, view.color, px(TILE_MARK), palette));
                // Named "Host settings · <name>", as the old dialog was, the name escaped; the heading shows the name
                // alone.
                let title = HostSentence::new("host-settings-title")
                    .text("Host settings · ")
                    .host(&display_name(&view));
                (title.into(), Some(heading(&view, palette)), tile, sections)
            }
            None => ("Host settings".into(), None, None, Vec::new()),
        };
        let body = div()
            .flex()
            .flex_col()
            .gap(px(SECTION_RULE_PAD))
            .text_color(theme::color(palette.fg_0))
            .children(body);
        let mut frame = DialogFrame::new("host-settings", title, body, vec![close])
            .note(FOOTER_NOTE)
            .width(px(WIDTH))
            .close(
                "Close host settings",
                HoverHelp::tooltip(CLOSE_HELP),
                false,
                on_dialog(weak, HostSettingsDialog::close),
            );
        if let Some(heading) = heading {
            frame = frame.heading(heading);
        }
        if let Some(tile) = tile {
            frame = frame.tile(tile);
        }
        frame
    }
}

impl HostSettingsDialog {
    /// SSH hosts can edit the address and its trust policy. Every host can change its alias.
    fn connection(
        &self,
        view: &HostView,
        kind: HostKind,
        palette: &Palette,
        cx: &Context<Self>,
    ) -> AnyElement {
        let mut rows = Vec::new();
        if kind.edits_destination() {
            rows.push(self.field_row(Field::Destination, view, palette, cx));
        }
        rows.push(self.field_row(Field::Alias, view, palette, cx));
        if kind.edits_destination() {
            rows.push(self.tailscale_row(view, palette, cx));
        }
        section("connection", "Connection", true, rows, palette)
    }

    /// A refused toggle returns to the pushed route and shows the reason on its own row.
    fn tailscale_row(&self, view: &HostView, palette: &Palette, cx: &Context<Self>) -> AnyElement {
        const HELP: &str = "Connect through Tailscale and verify the host's advertised SSH key instead of using \
                           ~/.ssh/known_hosts. Changing this setting reconnects the host.";
        let weak = cx.weak_entity();
        div()
            .flex()
            .flex_col()
            .gap(px(GROUP_GAP))
            .min_w_0()
            .child(
                div()
                    .text_size(px(LABEL_TEXT))
                    .line_height(px(LABEL_LINE_HEIGHT))
                    .child(
                        Checkbox::new(
                            TAILSCALE_SWITCH,
                            "Tailscale host",
                            HoverHelp::tooltip(HELP),
                            Role::Switch,
                        )
                        .switch()
                        .checked(shown(&self.tailscale.draft, view.tailscale))
                        .focus(&self.tailscale.focus)
                        .on_toggle(move |on, _, cx| {
                            let _ = weak.update(cx, |dialog, cx| dialog.toggle_tailscale(on, cx));
                        }),
                    ),
            )
            .child(help(HELP, palette))
            .children(self.tailscale.draft.refusal().map(|refusal| {
                failure_line("host-settings-tailscale-error".into(), refusal, palette)
            }))
            .children(self.tailscale_launch_refused.then(|| {
                failure_line(
                    "host-settings-tailscale-launch".into(),
                    &HostText::from_host(super::LAUNCH_IN_FLIGHT),
                    palette,
                )
            }))
            .children(self.refused.line(TAILSCALE_SWITCH, |id, text| {
                failure_line(id.into(), &HostText::from_host(text), palette)
            }))
            .into_any_element()
    }

    /// One Connection row (`.host-settings-row`): its label, then its value with Edit, or its open field with Save and
    /// Cancel under it; then the helm's words for the field's last refused save, until its next save starts.
    fn field_row(
        &self,
        field: Field,
        view: &HostView,
        palette: &Palette,
        cx: &Context<Self>,
    ) -> AnyElement {
        let weak = cx.weak_entity();
        let (label, edit_label, placeholder, edit_help, save_help) = match field {
            Field::Destination => (
                "Destination",
                "Edit destination",
                "SSH destination",
                "Edit: change the SSH destination Farhelm uses for this host",
                "Save the new SSH destination",
            ),
            Field::Alias => (
                "Alias",
                "Edit alias",
                "Host alias",
                "Edit: change the name this host is shown under",
                "Save the new alias",
            ),
        };
        let key = field.key();
        let content = if self.editing == Some(field) {
            let input = self.input(field);
            let saving = input.draft().is_saving();
            let mut text = TextInput::new(
                format!("host-settings-{key}-input"),
                input.input(),
                placeholder,
                HoverHelp::tooltip(format!("{placeholder}; Enter saves it")),
            );
            if field == Field::Destination {
                text = text.monospace();
            }
            let save = Button::new(
                format!("host-settings-{key}-save"),
                "Save",
                HoverHelp::tooltip(save_help),
                Role::Button,
            )
            .tier(Tier::Primary)
            .compact()
            .busy(saving)
            .focus(&self.save_focus)
            .on_click(move |_, window, cx| {
                let _ = weak.update(cx, |dialog, cx| dialog.save_field(field, window, cx));
            });
            let cancel = Button::new(
                format!("host-settings-{key}-cancel"),
                "Cancel",
                HoverHelp::tooltip("Cancel: keep the current value"),
                Role::Button,
            )
            .tier(Tier::Neutral)
            .compact()
            .disabled(saving)
            .focus(&self.cancel_focus)
            .on_click(on_dialog(cx.weak_entity(), HostSettingsDialog::cancel));
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(px(6.))
                .text_size(metrics::TEXT_SM)
                .child(text)
                .child(div().flex().gap(px(8.)).child(save).child(cancel))
                .into_any_element()
        } else {
            let value = match field.stored(view).map(HostText::from_host) {
                Some(value) => host_text::element(&value)
                    .when(field == Field::Destination, |value| {
                        value.font_family(TERMINAL_FONT)
                    }),
                None => div().child("none"),
            };
            // Named for what it edits, as the old buttons were, with "Edit" on the button: two buttons named "Edit"
            // would not say which value each changes. Unavailable while the other field is open (one setting at a
            // time, as the old dialog had it); focus is then in that field, which Edit moved it into.
            let edit = Button::new(
                format!("host-settings-edit-{key}"),
                edit_label,
                HoverHelp::tooltip(edit_help),
                Role::Button,
            )
            .content(div().child("Edit"))
            .compact()
            .disabled(self.editing.is_some())
            .focus(&self.edit_focus[field.index()])
            .on_click(move |_, window, cx| {
                let _ = weak.update(cx, |dialog, cx| dialog.edit(field, window, cx));
            });
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .items_center()
                .gap(px(ROW_GAP))
                .child(
                    value
                        .flex_1()
                        .min_w_0()
                        .text_color(theme::color(palette.fg_0)),
                )
                .child(div().flex_none().child(edit))
                .into_any_element()
        };
        let refusal = self.input(field).draft().refusal().map(|refusal| {
            failure_line(
                SharedString::from(format!("host-settings-{key}-error")),
                refusal,
                palette,
            )
            .ml(px(ROW_LABEL + ROW_GAP))
        });
        let busy = self.refused.line(field.control(), |id, text| {
            failure_line(id.into(), &HostText::from_host(text), palette).ml(px(ROW_LABEL + ROW_GAP))
        });
        let during_launch = field
            .waits_for_launches()
            .filter(|_| self.launch_refused == Some(field))
            .map(|prefix| {
                let words = format!("{prefix}{}", super::LAUNCH_IN_FLIGHT);
                failure_line(
                    SharedString::from(format!("host-settings-{key}-launch")),
                    &HostText::from_host(words),
                    palette,
                )
                .ml(px(ROW_LABEL + ROW_GAP))
            });
        div()
            .flex()
            .flex_col()
            .gap(px(6.))
            .child(
                // The label beside the value's line, or at the top of an open field's block (the old grid's
                // baseline alignment, which put it level with the field's first line).
                div()
                    .flex()
                    .when(self.editing == Some(field), |row| row.items_start())
                    .when(self.editing != Some(field), |row| row.items_center())
                    .gap(px(ROW_GAP))
                    .text_size(px(LABEL_TEXT))
                    .line_height(px(LABEL_LINE_HEIGHT))
                    .child(
                        div()
                            .flex_none()
                            .w(px(ROW_LABEL))
                            .text_color(theme::color(palette.fg_1))
                            .child(label),
                    )
                    .child(content),
            )
            .children(refusal)
            .children(during_launch)
            .children(busy)
            .into_any_element()
    }

    /// Appearance: the picker ([`appearance`]), its refusal line, and the line saying why a press sent nothing.
    fn appearance_section(
        &self,
        view: &HostView,
        palette: &Palette,
        cx: &Context<Self>,
    ) -> AnyElement {
        let weak = cx.weak_entity();
        let mut children = appearance::picker(
            shown(&self.appearance, (view.icon, view.color)),
            move |choice, _, cx| {
                let _ = weak.update(cx, |dialog, cx| dialog.choose(choice, cx));
            },
            palette,
        );
        children.extend(self.appearance.refusal().map(|refusal| {
            failure_line(format!("{APPEARANCE}-error").into(), refusal, palette).into_any_element()
        }));
        children.extend(self.refused.line(APPEARANCE, |id, text| {
            failure_line(id.into(), &HostText::from_host(text), palette).into_any_element()
        }));
        section("appearance", "Appearance", false, children, palette)
    }

    /// Permissions: each switch with its help and, after a refused save, the helm's words, then the line saying why a
    /// press sent nothing.
    fn permissions(&self, view: &HostView, palette: &Palette, cx: &Context<Self>) -> AnyElement {
        let switch_on = |permission: Permission| {
            shown(
                &self.switches[permission.index()].draft,
                permission.stored(view),
            )
        };
        let groups = Permission::ALL
            .iter()
            .map(|&permission| {
                let weak = cx.weak_entity();
                let switch = &self.switches[permission.index()];
                let failure = switch.draft.refusal().map(|refusal| {
                    failure_line(
                        SharedString::from(format!("{}-error", permission.id())),
                        refusal,
                        palette,
                    )
                });
                div()
                    .flex()
                    .flex_col()
                    .gap(px(GROUP_GAP))
                    .min_w_0()
                    .child(
                        div()
                            .text_size(px(LABEL_TEXT))
                            .line_height(px(LABEL_LINE_HEIGHT))
                            .child(
                                Checkbox::new(
                                    permission.id(),
                                    permission.label(),
                                    HoverHelp::tooltip(permission.tooltip()),
                                    Role::Switch,
                                )
                                .switch()
                                .checked(switch_on(permission))
                                .focus(&switch.focus)
                                .on_toggle(move |on, _, cx| {
                                    let _ = weak
                                        .update(cx, |dialog, cx| dialog.toggle(permission, on, cx));
                                }),
                            ),
                    )
                    .child(help(permission.help(switch_on), palette))
                    .children(failure)
                    .children(self.refused.line(&permission.id(), |id, text| {
                        failure_line(id.into(), &HostText::from_host(text), palette)
                    }))
                    .into_any_element()
            })
            .collect();
        section("permissions", "Permissions", false, groups, palette)
    }
}

/// The header's heading in the title's place (`HostDialogHeader`): the host's name, then its phase (the dot and words
/// in the phase's tone, as its row shows them) and, for a host that reports one, the build it runs.
fn heading(view: &HostView, palette: &Palette) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(px(6.))
        .child(
            host_text::element(&display_name(view))
                .truncate()
                .text_size(metrics::TEXT_TITLE)
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme::color(palette.fg_0)),
        )
        .child(
            div()
                .flex()
                .flex_wrap()
                .items_center()
                .gap_x(px(12.))
                .text_size(metrics::TEXT_SM)
                .text_color(theme::color(palette.fg_1))
                .child(
                    div()
                        .id("host-settings-status")
                        .role(Role::Status)
                        .aria_label(phase::words(&view.state))
                        .child(phase_dot_and_words(
                            &view.state,
                            phase::tone(&view.state),
                            1.,
                            palette,
                        )),
                )
                .children(phase::reported_build(&view.state).map(|build| {
                    HostSentence::new("host-settings-version")
                        .text("Farhelm ")
                        .mono(&HostText::from_host(build))
                })),
        )
}

/// A section (`.host-settings-section`): its small title, then `children`, 14 pixels apart. A section after the first
/// (`first` false) sits under a rule.
fn section(
    key: &'static str,
    title: &'static str,
    first: bool,
    children: Vec<AnyElement>,
    palette: &Palette,
) -> AnyElement {
    div()
        .id(SharedString::from(format!("host-settings-{key}")))
        .role(Role::Group)
        .aria_label(title)
        .flex()
        .flex_col()
        .gap(px(SECTION_GAP))
        .min_w_0()
        .when(!first, |section| {
            section
                .pt(px(SECTION_RULE_PAD))
                .border_t_1()
                .border_color(theme::color(palette.border_dim))
        })
        .child(
            div()
                .text_size(px(SECTION_TITLE))
                .font_weight(FontWeight::MEDIUM)
                .text_color(theme::color(palette.fg_1))
                .child(title),
        )
        .children(children)
        .into_any_element()
}

/// A help paragraph (`.host-settings-help`): 12 pixels in the quieter ink.
fn help(text: impl Into<SharedString>, palette: &Palette) -> Div {
    div()
        .text_size(metrics::TEXT_SM)
        .line_height(px(HELP_LINE_HEIGHT))
        .text_color(theme::color(palette.fg_1))
        .child(text.into())
}

/// The helm's words for a refused save, under the setting that made it (`.host-settings-outcome`), as an alert drawn
/// under `id` so tests and the test bridge find it beside its control. The words may quote host values, so they are host
/// text, one labelled node.
fn failure_line(id: SharedString, refusal: &HostText, palette: &Palette) -> Div {
    div()
        .text_size(metrics::TEXT_SM)
        .line_height(px(HELP_LINE_HEIGHT))
        .text_color(theme::color(palette.danger))
        .child(
            div()
                .id(gpui::ElementId::Name(format!("{id}-alert").into()))
                .role(Role::Alert)
                .child(HostSentence::new(gpui::ElementId::Name(id)).host(refusal)),
        )
}
