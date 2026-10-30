//! Edit launch templates without inventing values for fields they leave alone.
//!
//! The dialog keeps one explicit draft and a saved baseline. Stored kindless
//! templates are interpreted only in the draft: that difference is unsaved
//! until the user chooses save. API writes keep the existing last-write-wins
//! contract; name checks prevent an ordinary new/rename/undo from replacing a
//! template the user did not open.
//!
//! The shared frame does not split scrolling ownership: the dialog root keeps
//! the complete list/editor and its busy keyboard entry. Actions remain local
//! to the selected editor. Header Close and responsive Back both use the same
//! draft-departure guard, whose Save default is independent of answer order.

use super::shared::HostOption;
use crate::ApiBase;
use crate::anchored_popovers::{
    AnchorRegion, Interest, MovementOwner, Registration, use_registration,
};
use crate::api::{PreferenceValue, Preferences};
use crate::launch_sequence::TEMPLATE_SHORTCUTS;
use crate::menu_panel::{
    MenuFocusQueue, MenuOpenIntent, MenuOrder, MenuWiring, PanelPlacement, ROW_MENU_OUTSIDE_RELAY,
    cancel_menu_focus, closed_toggle_key_intent, focus_menu_toggle, forget_menu_focus,
    handle_menu_key, install_row_menu_outside_dismiss, measurement_outcome, record_menu_item_focus,
    remember_menu_item, session_menu_placement_style, use_menu_instance,
};
use crate::modal_dialog::{
    DialogActions, DialogFocus, DialogFooter, DialogHandle, DialogHeader, ModalDialog,
    RecoveryDelay, RecoveryFocus,
};
use crate::ops::{ConfirmSlot, ConfirmToken, use_confirm_slot};
use crate::peer::{DetailPart, PeerText, display_peer};
use dioxus::prelude::*;
pub(super) use farhelm_proto::launcher::quick_launch_eligible;
use farhelm_proto::launcher::{
    LaunchCatalogModel, LaunchTemplate, LauncherKind, TEMPLATE_NAME_CAP, TemplateDestination,
    TemplateFields, TemplateNameKind, check_template_shape,
};
use farhelm_proto::{LaunchHarness, LaunchPermission};
use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::rc::Rc;

/// The list owns actual close, while editor departures may keep this root mounted.
pub(super) const DIALOG_SELECTOR: &str = r#".templates-dialog[role="dialog"]"#;

/// Giving a key to a template moves it from its old owner and releases its
/// previous key. This whole-map edit preserves one key per template by construction.
/// Ownership compares original names; an escaped label is never a binding key.
fn bind_number(bindings: &mut BTreeMap<String, PeerText>, name: &str, key: Option<&str>) {
    bindings.retain(|_, owner| owner.raw() != name);
    if let Some(key) = key {
        bindings.insert(key.into(), name.into());
    }
}

/// Mirror fresh bindings without replacing unrelated cached preferences.
/// Callers apply the shared binding overlay first so unconfirmed local writes
/// retain their ownership; this refresh must never become a whole seed read.
pub(crate) fn apply_template_bindings(current: &mut Preferences, fresh: Preferences) {
    current.quick_launch_template = fresh.quick_launch_template;
    current.template_shortcuts = fresh.template_shortcuts;
}

/// Capture intent before reading; a stale displayed owner cannot decide a toggle.
///
/// `Bolt` carries what the user asked for at click time: `true` to mark the template (its bolt was unlit),
/// `false` to clear it (lit). The handler applies that to freshly read bindings and writes nothing when they
/// already agree, so a mark or move an agent made while the dialog was open is never undone.
enum BindingEdit {
    Bolt(String, bool),
    Number(String, Option<String>),
}

/// Move the old name's key even if the new name has a stale binding already.
/// A rename cannot produce two keys for one template, which the helm refuses.
/// Match the stored name byte-for-byte, independently of its display spelling.
fn rename_number_binding(bindings: &mut BTreeMap<String, PeerText>, old: &str, new: &str) {
    let key = bindings
        .iter()
        .find_map(|(key, name)| (name.raw() == old).then(|| key.clone()));
    if let Some(key) = key {
        bindings.remove(&key);
        bind_number(bindings, new, Some(&key));
    }
}

/// The rendered facts that can displace a template's number toggle.
///
/// Name and summary each occupy one nowrap line. Only the bolt chord adds a
/// line, and the toggle is centered in its own row. Counting earlier marked
/// rows preserves a harmless permutation above an unchanged anchor; comparing
/// the entire ordered prefix would close that anchor unnecessarily. The bound
/// value stays exact for prop updates and rendering; movement groups only
/// labels whose equal width is established by the current physical-key/font rules.
#[derive(Clone, Debug, PartialEq, Eq)]
struct TemplateAnchorKey {
    index: usize,
    marked_before: usize,
    marked: bool,
    bound: Option<String>,
}

impl TemplateAnchorKey {
    /// Separate strict presentation equality from possible anchor displacement.
    /// Known digits have distinct labels but equal width through the toggle's
    /// explicit tabular-numeral setting; changing one must still update the menu's
    /// binding without invalidating its placement.
    fn movement_key(&self) -> (usize, usize, bool, String) {
        (
            self.index,
            self.marked_before,
            self.marked,
            template_number_width_shape(self.bound.as_deref()),
        )
    }
}

/// Keep only proven equal-width digit chords equivalent for movement.
///
/// Use the actual formatter so physical-key remaps and fallback labels retain
/// their presentation. Normalize only a complete label matching a configured
/// single-ASCII-Digit chord; other Unicode, combining sequences and unknown
/// multi-character values retain their exact formatted strings. The unbound
/// placeholder includes its leading space and is not a one-digit chord.
fn template_number_width_shape(bound: Option<&str>) -> String {
    let label = TEMPLATE_SHORTCUTS.number_chord(bound.unwrap_or(" –"));
    let digit_chord = TEMPLATE_SHORTCUTS.number.keys.iter().any(|key| {
        key.code
            .strip_prefix("Digit")
            .is_some_and(|suffix| suffix.len() == 1 && suffix.as_bytes()[0].is_ascii_digit())
            && TEMPLATE_SHORTCUTS.number_chord(key.value) == label
    });
    if digit_chord {
        label
            .chars()
            .map(|glyph| if glyph.is_ascii_digit() { '0' } else { glyph })
            .collect()
    } else {
        label
    }
}

/// One saved row and the presentation values its markup and menus share.
/// The key is deliberately built from the actual received order and binding
/// lookup; it is not a second template catalog or a copy of draft state.
struct TemplateListRow {
    template: LaunchTemplate,
    anchor: TemplateAnchorKey,
}

/// Resolve row placement inputs once, then use them in both markup and admission.
/// An absent marked name or binding contributes nothing to the displayed list.
fn template_list_rows(
    rows: &[LaunchTemplate],
    marked_name: Option<&str>,
    bindings: &BTreeMap<String, PeerText>,
) -> Vec<TemplateListRow> {
    let mut marked_before = 0;
    rows.iter()
        .enumerate()
        .map(|(index, template)| {
            let marked = marked_name == Some(template.name.as_str());
            let bound = bindings
                .iter()
                .find_map(|(key, owner)| (owner.raw() == template.name).then(|| key.clone()));
            let anchor = TemplateAnchorKey {
                index,
                marked_before,
                marked,
                bound,
            };
            marked_before += usize::from(marked);
            TemplateListRow {
                template: template.clone(),
                anchor,
            }
        })
        .collect()
}

/// Retain the opening invalidated by a changed render until effects can run.
///
/// Dioxus can replace a queued effect's callback after another render. Keeping
/// this capture outside that callback preserves an A→B→A cause, while the
/// existing synchronous opening generation rejects a later same-row reopen.
/// Nothing survives this menu's component scope or replays through the owner.
struct TemplateAnchorMovement {
    previous: (usize, usize, bool, String),
    pending: Option<u64>,
}

impl TemplateAnchorMovement {
    /// Mounting establishes the first rendered baseline, not a movement event.
    fn new(previous: TemplateAnchorKey) -> Self {
        Self {
            previous: previous.movement_key(),
            pending: None,
        }
    }

    /// Capture before this synchronous component render commits its DOM edits.
    /// A later changed render may invalidate a newer opening, so replace an old
    /// capture only when there is a live opening at this new causal boundary.
    fn observe(&mut self, current: &TemplateAnchorKey, opening: Option<u64>) {
        let current = current.movement_key();
        if self.previous != current {
            self.previous = current;
            if let Some(opening) = opening {
                self.pending = Some(opening);
            }
        }
    }

    /// Drain once, and admit only the opening that existed before the change.
    fn take_for(&mut self, opening: Option<u64>) -> bool {
        let pending = self.pending.take();
        pending.is_some() && pending == opening
    }
}

/// Add row-content movement without changing the legacy epoch effect's policy.
///
/// Dioxus 0.7 runs component render bodies before emitting their mutations and
/// effects after the renderer applies them. Capture the existing generation in
/// that render body, not inside the deduplicated effect's replaceable callback.
/// Another popup on these rows can consume the same key and this local adapter;
/// the list/mark producer never enumerates popup kinds. This assumes the row
/// remains synchronous: adding suspense here requires revisiting the boundary.
fn use_template_anchor_movement(
    anchor: &TemplateAnchorKey,
    open: Signal<bool>,
    generation: Signal<u64>,
    registration: Registration,
) {
    let movement = use_hook(|| Rc::new(RefCell::new(TemplateAnchorMovement::new(anchor.clone()))));
    let opening = (*open.peek()).then(|| *generation.peek());
    movement.borrow_mut().observe(anchor, opening);
    let key = anchor.movement_key();
    use_effect(use_reactive((&key,), move |(_key,)| {
        let opening = (*open.peek()).then(|| *generation.peek());
        let admitted = movement.borrow_mut().take_for(opening);
        if admitted {
            registration.close_if_open();
        }
    }));
}

/// A row-local popup stays inside modal isolation while using the app's menu
/// keyboard, placement and outside-dismiss machinery. Escape closes this menu
/// without bubbling into the containing Templates dialog.
/// Option names stay concise; binding ownership and the consequence of clearing
/// a shortcut are visible and accessible descriptions on the same native items.
/// Those descriptions replace option tooltips; the toggle retains launch help.
#[component]
fn NumberShortcutMenu(
    name: String,
    anchor: TemplateAnchorKey,
    launchable: bool,
    disabled: bool,
    bindings: BTreeMap<String, PeerText>,
    layout_epoch: ReadSignal<u64>,
    dialog_epoch: ReadSignal<u64>,
    on_choose: EventHandler<Option<String>>,
) -> Element {
    let bound = anchor.bound.clone();
    let mut open = use_signal(|| false);
    let mut placement = use_signal(|| PanelPlacement::Unmeasured);
    let mut toggle = use_signal(|| None::<Rc<MountedData>>);
    let mut handles = use_signal(HashMap::new);
    let instance = use_menu_instance();
    let mut focused = use_signal(|| None::<usize>);
    let mut requested = use_signal(|| None::<usize>);
    let mut intent = use_signal(|| None::<MenuOpenIntent>);
    let focus = MenuFocusQueue {
        target: use_signal(|| None),
        in_flight: use_signal(|| None),
        draining: use_signal(|| false),
    };
    let mut generation = use_signal(|| 0_u64);
    let menu_key = format!("template-number:{name}");
    let focus_key = menu_key.clone();
    let close_menu = use_callback(move |()| {
        // An outside relay may arrive after another close across the renderer
        // bridge. Re-delivery must not return focus from a newer popup.
        if !*open.peek() {
            return;
        }
        let restore = focused.peek().is_some();
        open.set(false);
        cancel_menu_focus(focus);
        // The shared helper restores only menu-owned focus and prevents a
        // scroll back to this row after layout invalidation or an outside click.
        if restore {
            focus_menu_toggle("data-row-menu-key", &focus_key, ".templates-number-key");
        }
    });
    let anchor_name = name.clone();
    let registration = use_registration(
        use_context::<MovementOwner>(),
        Interest::new(AnchorRegion::Templates),
        move || (*open.peek()).then(|| anchor_name.clone()),
        move || close_menu.call(()),
    );
    use_template_anchor_movement(&anchor, open, generation, registration.clone());
    // Fixed coordinates become stale when either the dialog or its ancestors
    // move. Opening is deliberately not a dependency; the popup's own scroller
    // also leaves these epochs untouched, so its ten choices stay reachable.
    // Keep the existing initial run and both dependencies; unlike bar menus,
    // this continuation has no opening-epoch admission rule.
    use_effect(move || {
        layout_epoch();
        dialog_epoch();
        if *open.peek() {
            registration.close_if_open();
        }
    });
    // The popup and key listener must keep the same binding order. The first
    // action removes a binding; every later action comes from the definition.
    let order: MenuOrder<Option<&'static str>, 11> = MenuOrder::pack(
        std::array::from_fn(|index| {
            (index > 0).then(|| TEMPLATE_SHORTCUTS.number.keys[index - 1].value)
        }),
        |_| true,
    );
    let wiring = MenuWiring {
        order,
        handles,
        focus,
        generation,
        focused,
        requested,
        open_intent: intent,
        close_menu,
        instance,
    };
    let mut begin = move |opening: MenuOpenIntent| {
        generation += 1;
        let current = generation();
        placement.set(PanelPlacement::Unmeasured);
        handles.write().clear();
        cancel_menu_focus(focus);
        focused.set(None);
        requested.set(None);
        intent.set(Some(opening));
        install_row_menu_outside_dismiss();
        open.set(true);
        let handle = toggle.peek().clone();
        spawn(async move {
            let rect = match handle {
                Some(handle) => handle.get_client_rect().await.ok(),
                None => None,
            };
            if let Some(result) = measurement_outcome(current, *generation.peek(), rect) {
                placement.set(result);
            }
        });
    };
    let key_text = TEMPLATE_SHORTCUTS.number_chord(bound.as_deref().unwrap_or(" –"));
    let state = bound.as_ref().map_or_else(
        || "Click to give this template a number key.".into(),
        |key| {
            format!(
                "{} is set; click to change or remove it.",
                TEMPLATE_SHORTCUTS.number_chord(key)
            )
        },
    );
    let behavior: String = if launchable {
        "A number shortcut starts a session from this template right away, without opening New."
            .into()
    } else {
        format!(
            "This template cannot launch on its own. Its key opens New with it applied; {} starts a sequence to combine it with other templates.",
            TEMPLATE_SHORTCUTS
                .prefix
                .label(TEMPLATE_SHORTCUTS.mac_desktop)
        )
    };
    let tooltip = format!("{behavior} Only one template can hold each key. {state}");
    rsx! {
        span { class: "templates-number-control", "data-row-menu-key": "{menu_key}",
            button { r#type: "button", class: "templates-number-key font-mono",
                disabled,
                aria_label: "Number shortcut {display_peer(&name)}", aria_haspopup: "menu", aria_expanded: open(),
                "data-bound": bound.is_some(),
                "data-tooltip": "{tooltip}",
                onmounted: move |element| toggle.set(Some(element.data())),
                onfocusin: move |_| forget_menu_focus(wiring),
                onclick: move |_| { if open() { close_menu.call(()); } else { begin(MenuOpenIntent::First); } },
                onkeydown: move |event| {
                    if open() { event.stop_propagation(); handle_menu_key(&event, None, wiring, &()); }
                    else if let Some(intent) = closed_toggle_key_intent(&event.key()) {
                        event.prevent_default(); event.stop_propagation(); begin(intent);
                    }
                },
                "{key_text}"
            }
            button { class: ROW_MENU_OUTSIDE_RELAY, r#type: "button", hidden: true, tabindex: "-1",
                "data-row-menu": "{menu_key}", onclick: move |_| close_menu.call(()), }
            if open() {
                div { class: "host-row-menu-flyout templates-number-flyout", style: session_menu_placement_style(placement()),
                    div { class: "host-row-menu-panel",
                        crate::menu_panel::MenuHeader {
                            title_tooltip: display_peer(&name),
                            title: rsx! { span { class: "peer-value", dir: "ltr", "{display_peer(&name)}" } },
                            summary: rsx! { "Number shortcut" },
                        }
                        div { class: "session-row-menu-items", role: "menu", aria_label: "Number key for {display_peer(&name)}",
                            // A delayed outside relay can leave two menus mounted.
                            // Reuse the opaque mount identity so one menu cannot
                            // supply another's spoken ownership description.
                            for key in order.actions() {
                                button { r#type: "button", class: "btn menu-item session-row-menu-item templates-number-option", role: "menuitem",
                                    aria_label: key.map(|key| TEMPLATE_SHORTCUTS.number_chord(key)).unwrap_or_else(|| "No shortcut".into()),
                                    aria_describedby: format!("template-number-{instance}-{}-description", key.unwrap_or("none")),
                                    tabindex: if focused().and_then(|position| order.get(position)).or(order.get(0)) == Some(key) { "0" } else { "-1" },
                                    "data-menu-focus": crate::menu_panel::menu_item_focus_key(wiring, key),
                                    onmounted: move |element| remember_menu_item(wiring, key, element.data()),
                                    onfocusin: move |_| record_menu_item_focus(wiring, key), onfocusout: move |_| focused.set(None),
                                    onkeydown: move |event| { event.stop_propagation(); handle_menu_key(&event, order.position(key), wiring, &()); },
                                    onclick: move |_| { close_menu.call(()); on_choose.call(key.map(str::to_string)); },
                                    crate::icons::ActionIcon { glyph: crate::icons::ActionGlyph::Templates, class: "menu-icon session-row-menu-icon" }
                                    crate::menu_panel::MenuItemCopy {
                                        label: rsx! { if let Some(key) = key { span { class: "font-mono", "{TEMPLATE_SHORTCUTS.number_chord(key)}" } } else { "No shortcut" } },
                                        description_id: format!("template-number-{instance}-{}-description", key.unwrap_or("none")),
                                        description: rsx! {
                                            if let Some(key) = key {
                                                if let Some(owner) = bindings.get(key) {
                                                    span { class: "templates-number-owner peer-value", dir: "ltr",
                                                        if owner.raw() == name { "This template" } else { "{owner.escaped()} (moves here)" }
                                                    }
                                                } else { "No template uses this key" }
                                            } else { "Remove this template's number shortcut" }
                                        },
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
const DEFAULT: &str = "default";

/// Tell an already-open launcher to reload templates when the editor closes.
/// A deletion or edit otherwise leaves its old catalog usable behind the modal.
#[derive(Clone, Copy)]
pub(super) struct TemplatesRevision(pub(super) Signal<u64>);

/// Shared close schedules this only after an approved whole-dialog departure.
pub(super) const RETURN_FOCUS: &str =
    "document.querySelector('.templates-button')?.focus({ preventScroll: true });";

/// Move focus after responsive navigation has mounted its destination. A phone
/// row or Back button disappears when activated, so retaining focus there would
/// send keyboard users back to the beginning of the dialog.
fn focus_template_control(selector: &str) {
    let selector = serde_json::to_string(selector).expect("a string always serializes");
    document::eval(&format!(
        "requestAnimationFrame(() => document.querySelector('{DIALOG_SELECTOR}')?.querySelector({selector})?.focus())"
    ));
}

/// Recover focus lost to a disabled or removed control after a write or reload.
/// Reuse the dialog installer's focus memory, and preserve any usable chosen
/// owner, including exempt approval input. Two frames let explicit navigation
/// and prompt focus win before fallback.
fn restore_template_focus() {
    let recovery = template_recovery().script();
    document::eval(&format!(
        r#"requestAnimationFrame(() => requestAnimationFrame(() => {{
            const dialog = document.querySelector('{DIALOG_SELECTOR}');
            if (!dialog) return;
            {recovery}
        }}))"#,
    ));
}

/// Keep phone navigation and departure prompts ahead of fallback after writes.
fn template_recovery() -> RecoveryFocus {
    RecoveryFocus::new(
        ".templates-name",
        Some(".templates-new"),
        true,
        RecoveryDelay::TwoFrames,
    )
}

/// The same wire words serve as select values on both renderers.
pub(super) fn word<T: serde::Serialize>(value: T) -> String {
    match serde_json::to_value(value) {
        Ok(serde_json::Value::String(word)) => word,
        _ => String::new(),
    }
}

/// Controls offer only known enum words; a stale/unknown word remains a refusal.
fn from_word<T: serde::de::DeserializeOwned>(value: &str) -> Option<T> {
    serde_json::from_value(serde_json::Value::String(value.to_string())).ok()
}

/// Editor field identities separate absence from an empty text being edited.
/// Adding a field writes a present value immediately; removing it is the only
/// way to make its launcher edit absent again.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Field {
    Prompt,
    Agent,
    Model,
    Effort,
    Permissions,
    Wrapper,
    Container,
    Command,
    Yolo,
    Resume,
    Host,
    Destination,
    Name,
}

impl Field {
    // Command-only fields lead when present; their absence keeps the agent
    // editor in its usual agent/model/effort order without a second ordering table.
    pub(super) const ALL: [Self; 13] = [
        Self::Command,
        Self::Yolo,
        Self::Agent,
        Self::Model,
        Self::Effort,
        Self::Permissions,
        Self::Wrapper,
        Self::Container,
        Self::Resume,
        Self::Prompt,
        Self::Host,
        Self::Destination,
        Self::Name,
    ];

    /// Both stored approval fields have one user-facing name, but remain
    /// separate edits so switching kind cannot silently reinterpret either.
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Prompt => "Initial prompt",
            Self::Agent => "Agent type",
            Self::Model => "Model",
            Self::Effort => "Effort",
            Self::Permissions | Self::Yolo => "Approvals",
            Self::Wrapper => "Wrapper prefix",
            Self::Container => "Containerized",
            Self::Command => "Command",
            Self::Resume => "Resume command",
            Self::Host => "Host",
            Self::Destination => "Destination",
            Self::Name => "Session name",
        }
    }

    /// Stable field keys keep two different approvals controls distinguishable
    /// in the DOM and in tests even though their labels deliberately match.
    pub(super) fn key(self) -> &'static str {
        match self {
            Self::Prompt => "prompt",
            Self::Agent => "agent",
            Self::Model => "model",
            Self::Effort => "effort",
            Self::Permissions => "permissions",
            Self::Wrapper => "wrapper",
            Self::Container => "container",
            Self::Yolo => "yolo",
            Self::Command => "command",
            Self::Resume => "resume",
            Self::Host => "host",
            Self::Destination => "destination",
            Self::Name => "name",
        }
    }

    /// Presence, including a reset or an empty draft text, means this field
    /// will be applied; value truthiness must never decide whether it exists.
    pub(super) fn is_set(self, f: &TemplateFields) -> bool {
        match self {
            Self::Prompt => f.prompt.is_some(),
            Self::Agent => f.agent.is_some(),
            Self::Model => f.model.is_some(),
            Self::Effort => f.effort.is_some(),
            Self::Permissions => f.permissions.is_some(),
            Self::Wrapper => f.wrapper_prefix.is_some(),
            Self::Container => f.container.is_some(),
            Self::Command => f.command.is_some(),
            Self::Yolo => f.yolo.is_some(),
            Self::Resume => f.resume_command.is_some(),
            Self::Host => f.host.is_some(),
            Self::Destination => f.destination.is_some(),
            Self::Name => f.name.is_some(),
        }
    }

    /// Unsafe text is a display-only escape, never a replacement draft value.
    /// Single-line inputs can erase controls and conceal host-supplied model,
    /// folder or session-name text. Keep the raw seed untouched until the field
    /// is removed or replaced through another explicit control, including a model suggestion.
    fn escaped_read_only_value(self, f: &TemplateFields) -> Option<String> {
        if self == Self::Wrapper {
            let words = f.wrapper_prefix.as_ref()?.as_ref()?;
            let joined = shell_words::join(words);
            return joined
                .chars()
                .any(farhelm_proto::text::is_presentation_unsafe)
                .then(|| display_peer(&joined));
        }
        let value = match self {
            Self::Name => f.name.as_deref(),
            Self::Prompt => f.prompt.as_deref(),
            Self::Command => f.command.as_deref(),
            Self::Resume => f.resume_command.as_ref().and_then(|v| v.as_deref()),
            Self::Model => f.model.as_ref().and_then(|v| v.as_deref()),
            Self::Wrapper => unreachable!("wrapper has a word-wise display path"),
            Self::Destination => match &f.destination {
                Some(TemplateDestination::Folder(v) | TemplateDestination::Github(v)) => {
                    Some(v.as_str())
                }
                None => None,
            },
            _ => None,
        }?;
        value
            .chars()
            .any(|ch| {
                // Textareas preserve message line breaks and tabs. Invisible
                // formatting still uses the editor's raw-preserving escape.
                !(self == Self::Prompt && matches!(ch, '\n' | '\t'))
                    && farhelm_proto::text::is_presentation_unsafe(ch)
            })
            .then(|| display_peer(value))
    }

    /// Add an editable value without choosing another field on the user's
    /// behalf. In particular, model/effort do not require an agent type.
    fn add(self, f: &mut TemplateFields) {
        match self {
            Self::Prompt => f.prompt = Some(String::new()),
            Self::Agent => f.agent = Some(LaunchHarness::ALL[0]),
            Self::Model => f.model = Some(None),
            Self::Effort => f.effort = Some(None),
            Self::Permissions => f.permissions = Some(None),
            Self::Wrapper => f.wrapper_prefix = Some(None),
            Self::Container => f.container = Some(None),
            Self::Command => f.command = Some(String::new()),
            Self::Yolo => f.yolo = Some(false),
            Self::Resume => f.resume_command = Some(None),
            Self::Host => f.host = Some(String::new()),
            Self::Destination => f.destination = Some(TemplateDestination::Folder(String::new())),
            Self::Name => {
                f.name = Some(String::new());
                f.name_kind = None;
            }
        }
    }

    /// Removal leaves the field alone when the template is applied; it does
    /// not reset the launcher's current value. A removed name takes its kind
    /// with it, so re-adding the field cannot revive an unseen Fixed choice.
    pub(super) fn remove(self, f: &mut TemplateFields) {
        match self {
            Self::Prompt => f.prompt = None,
            Self::Agent => f.agent = None,
            Self::Model => f.model = None,
            Self::Effort => f.effort = None,
            Self::Permissions => {
                f.permissions = None;
                f.permission_flags = None;
            }
            Self::Wrapper => f.wrapper_prefix = None,
            Self::Container => f.container = None,
            Self::Command => f.command = None,
            Self::Yolo => f.yolo = None,
            Self::Resume => f.resume_command = None,
            Self::Host => f.host = None,
            Self::Destination => f.destination = None,
            Self::Name => {
                f.name = None;
                f.name_kind = None;
            }
        }
    }

    /// Decide which edits can be added under this switch, using harness-owned
    /// capability answers rather than a second list of per-agent facts.
    pub(super) fn offered(self, f: &TemplateFields) -> bool {
        match self {
            Self::Prompt => {
                f.kind == Some(LauncherKind::Agent)
                    && f.agent
                        .is_none_or(|agent| agent.supports_initial_prompt(false))
            }
            Self::Host | Self::Destination | Self::Name => true,
            Self::Agent => f.kind.is_some(),
            Self::Command | Self::Yolo | Self::Resume => f.kind == Some(LauncherKind::Command),
            Self::Model => {
                f.kind == Some(LauncherKind::Agent)
                    && f.agent.is_none_or(LaunchHarness::offers_model)
            }
            Self::Effort => {
                f.kind == Some(LauncherKind::Agent)
                    && f.agent.is_none_or(LaunchHarness::offers_effort)
            }
            Self::Permissions => {
                f.kind == Some(LauncherKind::Agent) && f.agent.is_none_or(|a| !a.offers_only_yolo())
            }
            Self::Wrapper | Self::Container => f.kind == Some(LauncherKind::Agent),
        }
    }

    /// Keep incompatible choices visible and require an explicit correction.
    /// Arbitrary model text is accepted even when the catalog does not know it;
    /// final launcher application retains its own stricter rules.
    fn refusal(self, f: &TemplateFields, catalog: Option<&[LaunchCatalogModel]>) -> Option<String> {
        let fits = if self == Self::Model {
            f.kind == Some(LauncherKind::Agent)
        } else {
            self.offered(f)
        };
        if !fits {
            return Some(
                "does not fit this launcher switch or agent type; remove it to save".into(),
            );
        }
        if self == Self::Container
            && let Some(Some(config)) = &f.container
        {
            return config.validate().err();
        }
        if self == Self::Permissions
            && let Some(Some(value)) = f.permissions
            && !permission_options(f.agent)
                .iter()
                .any(|(key, _)| *key == word(value))
        {
            return Some(
                "this agent type does not offer these approvals; change or remove them".into(),
            );
        }
        if self == Self::Effort
            && let Some(Some(value)) = f.effort
            && !effort_options(f, catalog)
                .iter()
                .any(|(key, _)| *key == word(value))
        {
            return Some(
                "this agent type or model does not offer this effort; change or remove it".into(),
            );
        }
        None
    }
}

/// The reset value follows the receiving harness's request default, while an
/// explicit supported permission remains saveable even when it currently has
/// that same effect. Hiding it would reject otherwise valid stored templates.
/// An unset agent offers every choice: partial templates defer compatibility
/// to the agent selected when they are applied, rather than guessing here.
fn permission_options(agent: Option<LaunchHarness>) -> Vec<(String, String)> {
    let mut options = vec![(
        DEFAULT.into(),
        format!(
            "{} (reset)",
            crate::launch_composer::permission_label(agent, None)
        ),
    )];
    options.extend(
        LaunchPermission::ALL
            .iter()
            .copied()
            .filter(|permission| agent.is_none_or(|a| a.offers_permission(*permission)))
            .map(|p| {
                (
                    word(p),
                    crate::launch_composer::permission_label(agent, Some(p)),
                )
            }),
    );
    options
}

/// Known catalogs narrow effort choices using the same helper as the launcher.
/// Before loading one, no catalog-based refusal is justified; the capability
/// check still rejects an agent type that has no effort choice at all.
fn effort_options(
    f: &TemplateFields,
    catalog: Option<&[LaunchCatalogModel]>,
) -> Vec<(String, String)> {
    let efforts = match (f.agent, catalog) {
        (Some(agent), Some(catalog)) => farhelm_proto::launcher::compatible_efforts(
            agent,
            f.model.as_ref().and_then(|m| m.as_deref()),
            catalog,
        ),
        _ => farhelm_proto::launcher::EFFORT_ORDER.to_vec(),
    };
    let mut options = vec![(DEFAULT.into(), "Default".into())];
    options.extend(efforts.into_iter().map(|v| {
        (
            word(v),
            crate::launch_composer::effort_label(Some(v)).into(),
        )
    }));
    options
}

/// The editor's draft uses the wire fields directly so absence, false, and
/// explicit null resets survive editing without a second translation model.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Draft {
    name: String,
    fields: TemplateFields,
    /// An incomplete shell quote stays visible and blocks save without
    /// replacing the last valid word list in the wire fields.
    wrapper_input: Option<String>,
    /// Keep an invalid port or incomplete path visible without losing the last
    /// valid wire choice. Save must refuse until corrected or removed.
    container_input: Option<crate::container_controls::ContainerDraft>,
}
impl Default for Draft {
    fn default() -> Self {
        Self {
            name: String::new(),
            fields: TemplateFields {
                kind: Some(LauncherKind::Agent),
                ..Default::default()
            },
            wrapper_input: None,
            container_input: None,
        }
    }
}

impl Draft {
    /// Normalize legacy choices only in the editor's draft. Comparing with
    /// the original baseline marks inferred kinds and dropped trust as unsaved;
    /// opening the editor never migrates stored records.
    fn from_template(t: &LaunchTemplate) -> Self {
        let mut fields = t.fields.clone();
        // Old templates decode their trust field, but Settings now owns it.
        // Clearing it in the draft drops it on edit, duplicate, and save.
        fields.workspace_trust = None;
        fields.kind = fields.kind.or(fields.implied_kind());
        Self {
            name: t.name.clone(),
            fields,
            wrapper_input: None,
            container_input: None,
        }
    }

    /// Refuse incompatible or unfinished fields without requiring a complete
    /// launch. Another stacked template may supply the missing agent or command.
    fn refusal(&self, catalog: Option<&[LaunchCatalogModel]>) -> Option<String> {
        if let Some(value) = &self.container_input {
            return value.config().err();
        }
        if let Some(text) = &self.wrapper_input {
            return Some(
                farhelm_proto::split_wrapper_prefix(text)
                    .err()
                    .map(crate::local_validation_notice)
                    .unwrap_or_else(|| {
                        "Wrapper prefix changed while saving; review it and try again".to_string()
                    }),
            );
        }
        if let Err(reason) = farhelm_proto::check_permission_flags(
            self.fields.permissions.flatten(),
            self.fields.permission_flags.as_deref(),
        ) {
            return Some(crate::local_validation_notice(reason));
        }
        for field in Field::ALL.into_iter().filter(|k| k.is_set(&self.fields)) {
            if let Some(message) = field.refusal(&self.fields, catalog) {
                return Some(format!("{}: {message}", field.label()));
            }
        }
        let f = &self.fields;
        for (label, value) in [
            ("command", f.command.as_deref()),
            ("model", f.model.as_ref().and_then(|v| v.as_deref())),
            (
                "resume command",
                f.resume_command.as_ref().and_then(|v| v.as_deref()),
            ),
            ("host", f.host.as_deref()),
            ("session name", f.name.as_deref()),
            (
                "destination",
                f.destination.as_ref().map(|v| match v {
                    TemplateDestination::Folder(s) | TemplateDestination::Github(s) => s.as_str(),
                }),
            ),
        ] {
            if value == Some("") {
                let verb = if label == "host" { "Choose" } else { "Type" };
                return Some(format!("{verb} the {label}, or remove its field"));
            }
        }
        None
    }
}

/// What the panel knows about the helm's template names at the moment of a
/// save.
#[derive(Debug, Clone, PartialEq, Eq)]
enum ListedNames<'a> {
    /// The list is being read: the first load, or a reload after a save, a
    /// delete or a retry. A reload keeps showing the previous list until it
    /// finishes, but that list may already be out of date (it lacks the
    /// template just saved), so it proves nothing about a name.
    Loading,
    /// The last read failed.
    Failed,
    /// The list as last read, with no read in progress.
    Loaded(Vec<&'a str>),
}

/// Why saving the form under `name` must not go ahead, or `None` when it may.
///
/// `previous` is the name of the template the form was opened from (`None`
/// for a new one), and `listed` is what the panel knows about the names.
///
/// Saving under the template's own name replaces that template, which is
/// the point of editing it (last write wins, SPEC.md). Any other save, a new
/// template or a rename, would silently replace whichever template already
/// has `name`, which SPEC_impl.md ("Templates") says the panel must refuse.
/// So it needs a freshly loaded list to show that the name is free: a list
/// being read, or one that failed to load, proves nothing, and treating it
/// as "not taken" is how a slow or failed read used to let a save overwrite
/// another template.
fn save_name_refusal(previous: Option<&str>, name: &str, listed: &ListedNames) -> Option<String> {
    if previous == Some(name) {
        return None;
    }
    match listed {
        ListedNames::Loading => Some(
            "The template list is still loading, so Farhelm cannot check that no other template has this name; \
             wait for it to load, then save again"
                .to_string(),
        ),
        ListedNames::Failed => Some(
            "The template list could not be loaded, so Farhelm cannot check that no other template has this name; \
             retry loading it above, then save again"
                .to_string(),
        ),
        ListedNames::Loaded(names) => names.contains(&name).then(|| {
            format!("A template named {name:?} already exists; edit that one, or choose another name")
        }),
    }
}

/// Give the launcher the editor's compatibility and unfinished-field rules.
/// Saving a partial setup need not make a complete launch, but must not store
/// a field combination the same editor would refuse.
pub(super) fn saved_fields_refusal(
    template: &LaunchTemplate,
    catalog: Option<&[LaunchCatalogModel]>,
) -> Option<String> {
    Draft {
        name: template.name.clone(),
        fields: template.fields.clone(),
        wrapper_input: None,
        container_input: None,
    }
    .refusal(catalog)
}

/// Check a new launcher save against the freshly read catalog of names.
/// A failed read cannot prove a name is free, and must never enable a write.
pub(super) fn new_name_refusal(name: &str, templates: Option<&[LaunchTemplate]>) -> Option<String> {
    let names = match templates {
        Some(templates) => ListedNames::Loaded(templates.iter().map(|t| t.name.as_str()).collect()),
        None => ListedNames::Failed,
    };
    save_name_refusal(None, name, &names)
}

/// Describe saved edits without flattening technical values into interface prose.
/// Command and resume bodies stay in the editor because they can be long.
/// Raw peer values remain isolated and escaped by the visible renderer; only
/// shell arguments and wrapper bytes use mono, not the surrounding labels.
fn template_summary_parts(fields: &TemplateFields) -> Vec<DetailPart> {
    let choice = |name: &str, value: Option<String>| match value {
        Some(value) => format!("{name} {value}"),
        None => format!("{name} default"),
    };
    let mut parts = Vec::new();
    let mut push = |runs: Vec<DetailPart>| {
        if !parts.is_empty() {
            parts.push(DetailPart::text(" · "));
        }
        parts.extend(runs);
    };
    if fields.prompt.is_some() {
        push(vec![DetailPart::text("Initial prompt")]);
    }
    if let Some(kind) = fields.kind {
        push(vec![DetailPart::text(format!(
            "{} launch",
            match kind {
                LauncherKind::Agent => "Agent",
                LauncherKind::Command => "Command",
            }
        ))]);
    }
    if let Some(agent) = fields.agent {
        push(vec![DetailPart::text(
            crate::launch_composer::harness_label(agent),
        )]);
    }
    if let Some(model) = &fields.model {
        push(vec![match model {
            Some(model) => DetailPart::peer(model.clone()),
            None => DetailPart::text("Model default"),
        }]);
    }
    if let Some(effort) = fields.effort {
        push(vec![DetailPart::text(choice(
            "Effort",
            effort.map(|effort| crate::launch_composer::effort_label(Some(effort)).to_string()),
        ))]);
    }
    if let Some(permissions) = fields.permissions {
        push(vec![DetailPart::text(format!(
            "Approvals {}",
            crate::launch_composer::permission_label(fields.agent, permissions)
        ))]);
    }
    if let Some(words) = &fields.permission_flags {
        push(vec![DetailPart::mono_peer(shell_words::join(words))]);
    }
    if let Some(prefix) = &fields.wrapper_prefix {
        push(match prefix {
            Some(words) => vec![
                DetailPart::text("Wrapper prefix "),
                DetailPart::mono_peer(shell_words::join(words)),
            ],
            None => vec![DetailPart::text("Wrapper prefix none")],
        });
    }
    if fields.command.is_some() {
        push(vec![DetailPart::text("Command")]);
    }
    if let Some(container) = &fields.container {
        push(vec![DetailPart::text(if container.is_some() {
            "Containerized"
        } else {
            "Containerized off"
        })]);
    }
    if let Some(yolo) = fields.yolo {
        push(vec![DetailPart::text(if yolo {
            "YOLO"
        } else {
            "Not YOLO"
        })]);
    }
    if let Some(resume) = &fields.resume_command {
        push(vec![DetailPart::text(if resume.is_some() {
            "Resume command"
        } else {
            "Resume default"
        })]);
    }
    if fields.host.is_some() {
        push(vec![DetailPart::text("Host")]);
    }
    if fields.destination.is_some() {
        push(vec![DetailPart::text("Destination")]);
    }
    if let Some(name) = &fields.name {
        push(vec![
            DetailPart::text("Name "),
            DetailPart::peer(name.clone()),
        ]);
    }
    if parts.is_empty() {
        vec![DetailPart::text("Sets nothing yet")]
    } else {
        parts
    }
}

/// Keep plain-text test expectations on the visible summary's escaped wording.
#[cfg(test)]
pub(super) fn template_summary(fields: &TemplateFields) -> String {
    crate::peer::detail_text(&template_summary_parts(fields))
}

/// Share inline summary markup between the template list and quick switcher.
/// The wrapper stays phrasing content inside their native buttons. Each peer
/// run owns its bidi boundary; technical runs additionally opt into mono.
#[component]
pub(super) fn TemplateSummary(class: String, fields: TemplateFields) -> Element {
    let parts = template_summary_parts(&fields);
    rsx! {
        span { class,
            for (index, part) in parts.iter().enumerate() {
                match part {
                    DetailPart::Text(text) => rsx! { span { key: "{index}", "{text}" } },
                    DetailPart::Peer(value) | DetailPart::MonoPeer(value) => rsx! {
                        bdi {
                            key: "{index}",
                            class: if matches!(part, DetailPart::MonoPeer(_)) { "peer-value font-mono" } else { "peer-value" },
                            dir: "ltr", "{value.escaped()}"
                        }
                    },
                }
            }
        }
    }
}

/// The same explanation accompanies the editor and launcher-save choices.
pub(super) const NAME_KIND_HELP: &str = "Base name: used as-is when free, otherwise numbered (app-review-2, -3, …). Fixed name: used exactly; a clash is reported.";

/// Base is represented by absence, so a saved draft and the helm's normalized
/// reply agree immediately. This control edits only the policy; its caller
/// keeps it coupled to a present name and owns the immutable save snapshot.
#[component]
pub(super) fn NameKindControl(
    kind: Option<TemplateNameKind>,
    disabled: bool,
    on_change: EventHandler<Option<TemplateNameKind>>,
) -> Element {
    let fixed = kind == Some(TemplateNameKind::Fixed);
    rsx! {
        div { class: "segmented-control template-name-kind", role: "group", aria_label: "Session name kind",
            for (value, text) in [(None, "Base name"), (Some(TemplateNameKind::Fixed), "Fixed name")] {
                button { r#type: "button", disabled, aria_pressed: fixed == value.is_some(),
                    "data-tooltip": "Session name: {text}. {NAME_KIND_HELP}",
                    onclick: move |_| on_change.call(value), "{text}"
                }
            }
        }
    }
}

/// Present the complete draft vocabulary without normalizing an old value.
///
/// Count after retaining an incompatible selection: two to five displayed
/// choices use shared segments, while other counts keep a native select.
/// Changing either presentation edits only this template's draft, including
/// native button Enter; saving and launching remain the caller's actions.
/// Select options also mark the saved selection because the controlled value
/// can arrive before its options mount. Both forms keep an explicit field name
/// independent of the option text, including peer-authored host names.
#[component]
fn Choice(
    label: String,
    value: String,
    mut options: Vec<(String, String)>,
    disabled: bool,
    on_change: EventHandler<String>,
) -> Element {
    if !options.iter().any(|(key, _)| key == &value) {
        options.push((value.clone(), format!("{value} (not offered)")));
    }
    rsx! {
        if (2..=5).contains(&options.len()) {
            div { class: "templates-field",
                span { "{label}" }
                div { class: "segmented-control template-choice", role: "group", aria_label: "{label}",
                    for (key, text) in options {
                        button { key: "{key}", r#type: "button", value: "{key}", disabled,
                            aria_pressed: key == value,
                            "data-tooltip": "{label}: the value this template applies",
                            onclick: move |_| on_change.call(key.clone()),
                            "{text}"
                        }
                    }
                }
            }
        } else {
            label { class: "templates-field",
                span { "{label}" }
                select { disabled, value: "{value}", aria_label: "{label}", "data-tooltip": "{label}: the value this template applies",
                    onchange: move |event| on_change.call(event.value()),
                    for (key, text) in options { option { value: "{key}", selected: key == value, "{text}" } }
                }
            }
        }
    }
}

/// A present field's controls, independent of list navigation and save state.
/// Changes only update the draft; they never clear another field to make it fit.
/// Text fields keep literal commands, paths, model IDs and chosen names:
/// browser correction and remembered form values must not rewrite them.
/// Unsafe model, destination, command and session-name text is escaped and
/// read-only so a browser edit cannot silently change its bytes. The draft keeps the original
/// until an explicit replacement; old commands still reach save validation raw.
#[component]
fn FieldEditor(
    field: Field,
    mut draft: Signal<Draft>,
    hosts: Vec<HostOption>,
    catalog: Option<Vec<LaunchCatalogModel>>,
    disabled: bool,
) -> Element {
    let current = draft().fields;
    let escaped = field.escaped_read_only_value(&current);
    let text_read_only = escaped.is_some();
    let flags_seed = current
        .permission_flags
        .as_ref()
        .map(shell_words::join)
        .unwrap_or_default();
    let mut permission_flags_text = use_signal(move || flags_seed);
    let refusal = field
        .refusal(&current, catalog.as_deref())
        .map(crate::local_validation_notice);
    let label = field.label();
    // These are fixed application nouns. Standalone headings start a sentence,
    // while Remove and Add compose the same names into an existing sentence.
    let noun = label.to_ascii_lowercase();
    let controls = match field {
        Field::Container => rsx! {
            crate::container_controls::ContainerControls {
                value: draft().container_input.unwrap_or_else(|| crate::container_controls::ContainerDraft::from_config(current.container.as_ref().and_then(Option::as_ref))),
                busy: disabled,
                on_change: move |edit: crate::container_controls::ContainerEdit| {
                    if disabled { return; }
                    draft.with_mut(|d| {
                        let mut value = d.container_input.take().unwrap_or_else(|| crate::container_controls::ContainerDraft::from_config(d.fields.container.as_ref().and_then(Option::as_ref)));
                        value.apply(edit);
                        match value.config() {
                            Ok(config) => { d.fields.container = Some(config); }
                            Err(_) => d.container_input = Some(value),
                        }
                    });
                },
            }
        },
        Field::Wrapper => {
            let value = escaped.unwrap_or_else(|| {
                draft().wrapper_input.unwrap_or_else(|| {
                    current
                        .wrapper_prefix
                        .clone()
                        .flatten()
                        .map(|words| shell_words::join(&words))
                        .unwrap_or_default()
                })
            });
            rsx! {
                label { class: "templates-field",
                    span { "Wrapper prefix" }
                    input { class: "templates-wrapper templates-code", r#type: "text", value,
                        disabled, readonly: text_read_only, autocomplete: "off", autocorrect: "off", autocapitalize: "none", spellcheck: "false",
                        placeholder: "with-endpoint --profile work --",
                        "data-tooltip": "Wrapper prefix: words placed before Farhelm's agent command; no placeholders",
                        oninput: move |event| {
                            let text = event.value();
                            if !text_read_only { draft.with_mut(|d| {
                                if text.is_empty() {
                                    d.fields.wrapper_prefix = Some(None);
                                    d.wrapper_input = None;
                                } else {
                                    match farhelm_proto::split_wrapper_prefix(&text) {
                                        Ok(words) => {
                                            d.fields.wrapper_prefix = Some(Some(words));
                                            d.wrapper_input = None;
                                        }
                                        Err(_) => d.wrapper_input = Some(text),
                                    }
                                }
                            }); }
                        },
                    }
                }
                span { class: "templates-hint", "An empty input removes the wrapper. The prefix runs before Farhelm's agent command." }
            }
        }
        Field::Prompt => rsx! {
            label { class: "templates-field",
                span { "Initial prompt" }
                textarea { class: "templates-prompt", rows: 4,
                    value: escaped.unwrap_or_else(|| current.prompt.clone().unwrap_or_default()),
                    disabled, readonly: text_read_only,
                    autocomplete: "off", autocorrect: "off", autocapitalize: "none", spellcheck: "false",
                    "data-tooltip": "Initial prompt: the first message on each launch from this template",
                    oninput: move |event| { if !text_read_only { draft.with_mut(|draft| draft.fields.prompt = Some(event.value())); } },
                }
            }
            span { class: "templates-hint", "Sent as the agent's first message on every launch from this template. You can still edit it in the launcher before launching." }
        },
        Field::Agent => rsx! {
            Choice { label, value: current.agent.map(word).unwrap_or_default(),
                options: LaunchHarness::ALL.iter().map(|a| (word(*a), crate::launch_composer::harness_label(*a).to_string())).collect(), disabled,
                on_change: move |v: String| draft.with_mut(|d| d.fields.agent = from_word(&v)),
            }
        },
        Field::Model => {
            let value =
                escaped.unwrap_or_else(|| current.model.clone().flatten().unwrap_or_default());
            // Show alternatives even for a saved or custom model. Filtering by
            // the current value would hide the catalog unless the user cleared it.
            let suggestions: Vec<_> = current
                .agent
                .map(|a| {
                    crate::launch_composer::model_options(
                        catalog.as_deref().unwrap_or_default(),
                        Some(a),
                        "",
                        false,
                    )
                    .into_iter()
                    .filter_map(|option| match option {
                        crate::launch_composer::ModelOption::Model { id, .. } => Some(id),
                        _ => None,
                    })
                    .collect()
                })
                .unwrap_or_default();
            rsx! {
                label { class: "templates-field",
                    span { "Model" }
                    input { class: "templates-model", r#type: "text", value: "{value}", disabled, readonly: text_read_only, placeholder: "Default, or type any model",
                        autocomplete: "off", autocorrect: "off", autocapitalize: "none", spellcheck: "false",
                        "data-tooltip": "Model: type any model; suggestions follow the agent type this template sets",
                        oninput: move |event| { if !text_read_only { draft.with_mut(|d| d.fields.model = Some((!event.value().is_empty()).then(|| event.value()))); } },
                    }
                }
                if !suggestions.is_empty() {
                    div { class: "templates-model-suggestions", role: "group", aria_label: "Model suggestions",
                        for id in suggestions {
                            button { r#type: "button", class: "templates-suggestion", disabled,
                                "data-tooltip": "Set model to {display_peer(&id)}", onclick: move |_| draft.with_mut(|d| d.fields.model = Some(Some(id.clone()))), "{display_peer(&id)}"
                            }
                        }
                    }
                }
                span { class: "templates-hint", "An empty input sets the agent's default model. Any other model text is kept." }
            }
        }
        Field::Effort => rsx! {
            Choice { label, value: current.effort.flatten().map(word).unwrap_or_else(|| DEFAULT.into()), options: effort_options(&current, catalog.as_deref()), disabled,
                on_change: move |v: String| draft.with_mut(|d| d.fields.effort = Some(from_word(&v))),
            }
        },
        Field::Permissions => rsx! {
            Choice { label, value: current.permissions.flatten().map(word).unwrap_or_else(|| DEFAULT.into()), options: permission_options(current.agent), disabled,
                on_change: move |v: String| draft.with_mut(|d| {
                    d.fields.permissions = Some(from_word(&v));
                    d.fields.permission_flags = crate::launch_composer::permission_words(d.fields.permissions.flatten(), &permission_flags_text());
                    if d.fields.permissions.flatten() != Some(LaunchPermission::Custom) { permission_flags_text.set(String::new()); }
                }),
            }
            if current.permissions.flatten() == Some(LaunchPermission::Custom) {
                crate::launch_controls::CustomPermissionFlags { text: permission_flags_text(), busy: disabled, harness: current.agent,
                    on_change: move |text: String| {
                        draft.with_mut(|d| d.fields.permission_flags = farhelm_proto::split_permission_flags(&text).ok());
                        permission_flags_text.set(text);
                    },
                }
            }
        },
        Field::Yolo => rsx! {
            Choice { label, value: current.yolo.map(|v| v.to_string()).unwrap_or_default(),
                options: vec![("false".into(), "Keeps safety checks".into()), ("true".into(), "YOLO".into())], disabled,
                on_change: move |v: String| draft.with_mut(|d| d.fields.yolo = v.parse().ok()),
            }
            span { class: "templates-hint", "Your statement about this command; Farhelm does not inspect it." }
        },
        Field::Command => rsx! {
            label { class: "templates-field",
                span { "Command" }
                input { class: "templates-command templates-code", value: escaped.unwrap_or_else(|| current.command.unwrap_or_default()), disabled, readonly: text_read_only, placeholder: "agent-tool {{farhelm_args}}",
                    autocomplete: "off", autocorrect: "off", autocapitalize: "none", spellcheck: "false",
                    "data-tooltip": "Command: the command this template puts in the launcher", oninput: move |e| { if !text_read_only { draft.with_mut(|d| d.fields.command = Some(e.value())); } },
                }
            }
            span { class: "templates-hint", "Placeholders: {{cwd}} for the folder and {{farhelm_args}} for Farhelm's integration arguments; each must be a whole argument." }
        },
        Field::Resume => rsx! {
            label { class: "templates-field",
                span { "Resume command" }
                input { class: "templates-resume templates-code", value: escaped.unwrap_or_else(|| current.resume_command.flatten().unwrap_or_default()), disabled, readonly: text_read_only, placeholder: "Default, or agent-tool --resume {{conversation}} {{farhelm_args}}",
                    autocomplete: "off", autocorrect: "off", autocapitalize: "none", spellcheck: "false",
                    "data-tooltip": "Resume command: an empty input resets it to the default", oninput: move |e| { if !text_read_only { draft.with_mut(|d| d.fields.resume_command = Some((!e.value().is_empty()).then(|| e.value()))); } },
                }
            }
            span { class: "templates-hint", "An empty input sets the default. Placeholders: {{cwd}}, {{farhelm_args}}, {{conversation}}; each must be a whole argument." }
        },
        Field::Host => {
            let value = current.host.unwrap_or_default();
            let mut options: Vec<_> = hosts
                .into_iter()
                .filter_map(|h| h.identity.map(|id| (id, display_peer(&h.name))))
                .collect();
            if value.is_empty() {
                options.insert(0, (String::new(), "Choose a host".into()));
            } else if !options.iter().any(|(id, _)| id == &value) {
                options.push((value.clone(), "An install no host here has".into()));
            }
            rsx! { Choice { label, value, options, disabled, on_change: move |v: String| draft.with_mut(|d| d.fields.host = Some(v)) } }
        }
        Field::Name => rsx! {
            div { class: "templates-field",
                div { class: "template-name-label",
                    label { r#for: "template-session-name", "Session name" }
                    NameKindControl { kind: current.name_kind, disabled,
                        on_change: move |kind| draft.with_mut(|d| d.fields.name_kind = kind)
                    }
                }
                input { id: "template-session-name", aria_label: "Session name", class: "templates-session-name", value: escaped.unwrap_or_else(|| current.name.unwrap_or_default()), disabled, readonly: text_read_only, placeholder: "review",
                    autocomplete: "off", autocorrect: "off", autocapitalize: "none", spellcheck: "false",
                    "data-tooltip": "Session name: the name this template puts in the launcher", oninput: move |e| { if !text_read_only { draft.with_mut(|d| d.fields.name = Some(e.value())); } },
                }
                p { class: "templates-hint", "{NAME_KIND_HELP}" }
            }
        },
        Field::Destination => {
            let (github, value) = match current.destination {
                Some(TemplateDestination::Github(v)) => (true, v),
                Some(TemplateDestination::Folder(v)) => (false, v),
                None => (false, String::new()),
            };
            rsx! {
                div { class: "templates-field",
                    span { "Destination" }
                    crate::launch_controls::DestinationTypeControl { managed: github, disabled,
                        on_change: move |is_github| draft.with_mut(|d| {
                            let value = match d.fields.destination.take() {
                                Some(TemplateDestination::Folder(v) | TemplateDestination::Github(v)) => v,
                                None => String::new(),
                            };
                            d.fields.destination = Some(if is_github { TemplateDestination::Github(value) } else { TemplateDestination::Folder(value) });
                        }),
                    }
                    label {
                        span { if github { "Repository" } else { "Folder path" } }
                        input { class: if github { "templates-destination" } else { "templates-destination templates-code" }, value: escaped.unwrap_or(value), disabled, readonly: text_read_only, placeholder: if github { "owner/name" } else { "/path/to/project" },
                            autocomplete: "off", autocorrect: "off", autocapitalize: "none", spellcheck: "false",
                            "data-tooltip": if github { "Repository: each session gets a managed checkout" } else { "Folder path: the working folder on the selected host" },
                            oninput: move |e| { if !text_read_only { draft.with_mut(|d| d.fields.destination=Some(if github { TemplateDestination::Github(e.value()) } else { TemplateDestination::Folder(e.value()) })); } },
                        }
                    }
                }
                if github { span { class: "templates-hint", "Each session from this template gets its own managed checkout. Farhelm clones the repository into a new folder named after the session, under the host's checkout folder. When the last session using it is deleted, the folder moves to the trash." } }
            }
        }
    };
    rsx! {
        div { class: if refusal.is_some() { "templates-set-field templates-incompatible" } else { "templates-set-field" }, "data-field": field.key(),
            div { class: "templates-field-controls", {controls}
                if text_read_only { p { class: "templates-hint templates-value-readonly", "This field contains characters this editor cannot show directly; they are shown as escapes. Remove this field with ✕ and add it again to replace its value." } }
                if let Some(ref message)=refusal { p { class: "templates-error", "{message}" } }
            }
            button { class: "templates-remove", r#type: "button", disabled, aria_label: "Remove {noun}", "data-tooltip": "Remove {noun}: this template will leave it as is",
                onclick: move |_| draft.with_mut(|d| {
                    field.remove(&mut d.fields);
                    if field == Field::Wrapper { d.wrapper_input = None; }
                    if field == Field::Container { d.container_input = None; }
                }), "✕"
            }
        }
    }
}

/// All editor departure paths share the same guard, including mobile Back and
/// Escape. A departure held by the "Save these changes before leaving?"
/// prompt is that prompt's [`ConfirmSlot`] key: it carries its destination,
/// so saving from the prompt still goes where the user was heading, and each
/// answer carries the departure its own render drew.
#[derive(Clone, PartialEq, Debug)]
enum Departure {
    // Only Open carries a stored template. Keep the prompt key (cloned into
    // every token and answer) small as template fields grow; the other
    // departures carry no payload.
    Open(Box<LaunchTemplate>),
    New,
    Duplicate,
    Back,
    Close,
}

/// A free copy name is only a draft suggestion; the save still checks the
/// loaded list. Truncate at UTF-8 boundaries to respect the API's byte limit.
fn duplicate_name(name: &str, names: &[String]) -> String {
    for number in 1.. {
        let suffix = if number == 1 {
            " copy".to_string()
        } else {
            format!(" copy {number}")
        };
        let mut end = name.len().min(TEMPLATE_NAME_CAP - suffix.len());
        while !name.is_char_boundary(end) {
            end -= 1;
        }
        let candidate = format!("{}{suffix}", &name[..end]);
        if !names.contains(&candidate) {
            return candidate;
        }
    }
    unreachable!()
}

/// Apply an approved departure only after the dirty draft has been handled.
/// Duplicate copies the saved baseline, so discard cannot sneak unsaved edits
/// into a new template. The caller refreshes that baseline before save-and-leave.
#[allow(clippy::too_many_arguments)]
fn leave_editor(
    action: Departure,
    mut draft: Signal<Draft>,
    mut baseline: Signal<Draft>,
    mut previous: Signal<Option<String>>,
    mut opened: Signal<bool>,
    mut leave_prompt: ConfirmSlot<Departure>,
    mut error: Signal<Option<String>>,
    names: Vec<String>,
    on_close: EventHandler<()>,
) {
    let focus = match &action {
        Departure::Back => Some(".templates-new"),
        Departure::Close => None,
        _ => Some(".templates-name"),
    };
    // Reconciliation, not an answer: whichever path got here (an answer that
    // already took or accepted the prompt's opening, a clean draft that never
    // asked, a delete), the editor is moving on and no prompt describes it.
    leave_prompt.clear();
    error.set(None);
    match action {
        Departure::Open(template) => {
            baseline.set(Draft {
                name: template.name.clone(),
                fields: template.fields.clone(),
                wrapper_input: None,
                container_input: None,
            });
            draft.set(Draft::from_template(&template));
            previous.set(Some(template.name));
            opened.set(true);
        }
        Departure::New => {
            draft.set(Draft::default());
            baseline.set(Draft::default());
            previous.set(None);
            opened.set(true);
        }
        Departure::Duplicate => {
            let stored = baseline();
            let mut copy = Draft::from_template(&LaunchTemplate {
                name: stored.name,
                fields: stored.fields,
            });
            copy.name = duplicate_name(&copy.name, &names);
            // The source is already stored; a new copy must be saved explicitly
            // even if none of its values has been edited yet.
            baseline.set(Draft::default());
            draft.set(copy);
            previous.set(None);
            opened.set(true);
        }
        Departure::Back => {
            opened.set(false);
            draft.set(Draft::default());
            baseline.set(Draft::default());
            previous.set(None);
        }
        Departure::Close => on_close.call(()),
    }
    if let Some(selector) = focus {
        focus_template_control(selector);
    }
}

// The "Save these changes before leaving?" prompt's answers.
//
// The prompt is a `ConfirmSlot<Departure>`: every departure from a dirty
// draft opens a new opening keyed by where the user was heading, and the
// render that draws the prompt moves that opening's token into its three
// buttons. Each answer below acts only for the token it was drawn with and
// only on the departure that token names, never on the slot as it is when
// the click runs. A click drawn for an earlier showing (one the user since
// answered with "keep editing", or one another departure replaced while the
// prompt was up) does nothing and leaves the prompt now on screen alone.
//
// Save is the one answer that does not consume its opening: the prompt stays
// up, buttons disabled, while the save runs, and stays up after a failed save
// (the error shows in the dialog's status line, not in the prompt) so the
// user can answer the same question again; a successful save retires it when
// `leave_editor` moves on. That is the stay-up-through-the-request exception
// `ConfirmSlot`'s docs describe, and the dialog's `busy` flag is what refuses
// every other answer meanwhile. Save itself refuses on `busy`, and Discard
// and "keep editing" take `busy` too: they are refused while the dialog is
// busy with a write (a save from this prompt included, but also, for
// example, a binding edit or an Undo started elsewhere in the dialog), which
// is also when all three buttons are drawn disabled. Without that refusal, a
// Discard queued behind the prompt's "save" in one burst would run its
// departure at once and the save's success would run the same departure a
// second time, and a "keep editing" queued the same way would close the
// prompt only for the save's success to leave the editor anyway, silently
// overriding the user's answer.

/// The departure a "save" click drawn for `shown` saves and then performs,
/// or `None` when `shown` is no longer the open showing.
///
/// Leaves the opening open (see the section comment above for why). The
/// caller must already have refused the click while the dialog is busy with
/// a write.
fn answer_save_and_leave(
    slot: &ConfirmSlot<Departure>,
    shown: &ConfirmToken<Departure>,
) -> Option<Departure> {
    slot.is_current(shown).then(|| shown.key().clone())
}

/// The departure a "discard" click drawn for `shown` performs, consuming the
/// opening, or `None` (and the prompt unchanged) when `shown` is no longer
/// the open showing or the dialog is busy with a write (`busy`; a save from
/// this prompt included).
///
/// A stale Discard is the case that matters: one handled after "keep
/// editing" closed the prompt would throw away the draft the user had just
/// chosen to keep. The `busy` refusal keeps a Discard queued behind the
/// prompt's "save" from leaving now and again when that save succeeds.
fn answer_discard(
    slot: &mut ConfirmSlot<Departure>,
    shown: &ConfirmToken<Departure>,
    busy: bool,
) -> Option<Departure> {
    if busy {
        return None;
    }
    slot.take(shown).map(|()| shown.key().clone())
}

/// Whether a "keep editing" click drawn for `shown` closes the prompt (and
/// the caller moves focus to the name field): only when `shown` is the open
/// showing and the dialog is not busy with a write (`busy`; a save from this
/// prompt included). A refused click changes nothing, focus included.
///
/// The `busy` refusal matters because accepting "keep editing" behind a
/// running save from the prompt would close the prompt, and the save's
/// success would then leave the editor regardless.
fn answer_keep_editing(
    slot: &mut ConfirmSlot<Departure>,
    shown: &ConfirmToken<Departure>,
    busy: bool,
) -> bool {
    !busy && slot.take(shown).is_some()
}

/// A deleted template lives only in this mounted dialog. The generation keeps
/// an older expiry timer from clearing a later deletion's undo notice.
#[derive(Clone, PartialEq)]
struct Undo {
    template: LaunchTemplate,
    generation: u64,
}

/// Which Delete the editor's button offers for the open template: an ordinary
/// one with a ten-second Undo, or a permanent one. The button's label and
/// tooltip state the kind, so the click must carry out the kind they stated
/// (`undo_after_delete`) rather than work it out again at click time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DeleteKind {
    /// Undo is offered after the delete succeeds.
    Restorable,
    /// No Undo: the stored template fails today's shape rules, and Undo
    /// restores through the ordinary save path, which refuses such a value.
    Permanent,
}

impl DeleteKind {
    /// Classify the stored template named `previous` in the listed rows.
    ///
    /// The stored snapshot decides, not the editor's draft: Delete removes
    /// the stored value even when unsaved edits have repaired it, and Undo
    /// would have to save that original. A name missing from the rows (or no
    /// list at all) reads as `Restorable`; the click then refuses on its own
    /// because it has no stored template to snapshot.
    fn of(rows: Option<&[LaunchTemplate]>, previous: Option<&str>) -> Self {
        let stored_is_unsaveable = rows
            .and_then(|rows| rows.iter().find(|t| Some(t.name.as_str()) == previous))
            .is_some_and(|t| check_template_shape(t).is_err());
        if stored_is_unsaveable {
            DeleteKind::Permanent
        } else {
            DeleteKind::Restorable
        }
    }
}

/// Keep each destructive promise attached to the element the user can click.
/// Native Dioxus replaces listeners before the webview paints the mutations:
/// changing one button's kind could execute Permanent under a painted plain
/// Delete label. Both controls stay mounted, and each always sends its own
/// literal kind, so a click from the preceding paint keeps that promise.
#[component]
fn TemplateDeleteButtons(
    kind: DeleteKind,
    disabled: bool,
    on_delete: EventHandler<DeleteKind>,
) -> Element {
    rsx! {
        button {
            r#type: "button", class: "btn btn-danger templates-delete templates-delete-restorable",
            hidden: kind != DeleteKind::Restorable, disabled,
            "data-tooltip": "Delete this template now; undo is available here for about ten seconds",
            onclick: move |_| on_delete.call(DeleteKind::Restorable),
            "Delete"
        }
        button {
            r#type: "button", class: "btn btn-danger templates-delete templates-delete-permanent",
            hidden: kind != DeleteKind::Permanent, disabled,
            "data-tooltip": "Delete this template now; it fails current save rules, so undo is not available",
            onclick: move |_| on_delete.call(DeleteKind::Permanent),
            "Delete permanently"
        }
    }
}

/// The Undo notice a successful Delete leaves, decided by the kind the button
/// showed, never by the template value the click went on to delete.
///
/// The label is drawn from one render's template list while the click reads
/// the list as it is when the click is handled. If the two disagree, the
/// user agreed to what the label said: "delete permanently" gets no Undo
/// even if the deleted value would now pass the save rules, and "delete" gets
/// its Undo. In that second case the Undo can fail, because the helm's save
/// refuses a value outside the shape rules; the refusal shows as the
/// dialog's ordinary error and the template stays deleted, which is where a
/// permanent delete would have left it.
fn undo_after_delete(shown: DeleteKind, template: LaunchTemplate, generation: u64) -> Option<Undo> {
    (shown == DeleteKind::Restorable).then_some(Undo {
        template,
        generation,
    })
}

/// Consult the resource's pending state before its retained value: a reload
/// still displays an old list, which cannot establish that a new name is free.
fn name_refusal(
    templates: Resource<Result<Vec<LaunchTemplate>, String>>,
    previous: Option<&str>,
    name: &str,
) -> Option<String> {
    let listed = templates.peek();
    let names = match listed.as_ref() {
        _ if templates.pending() => ListedNames::Loading,
        None => ListedNames::Loading,
        Some(Err(_)) => ListedNames::Failed,
        Some(Ok(list)) => ListedNames::Loaded(list.iter().map(|t| t.name.as_str()).collect()),
    };
    save_name_refusal(previous, name, &names)
}

/// Manage the same modal lifecycle as the launcher expects, with one local
/// draft and one serialized write at a time. Async save/delete/undo work keeps
/// navigation disabled until its outcome is known; failed writes keep the draft.
#[component]
pub(super) fn TemplatesDialog(
    hosts: Vec<HostOption>,
    layout_epoch: ReadSignal<u64>,
    on_close: EventHandler<()>,
    /// A successful launcher save opens its actual returned fields directly;
    /// the asynchronous list read is not needed to rediscover the new record.
    initial_template: Option<LaunchTemplate>,
    /// A shortcut with no usable mark explains how to enable it here.
    #[props(default)]
    quick_launch_hint: bool,
    /// An unbound number chord explains row binding rather than the bolt.
    #[props(default)]
    number_shortcut_hint: bool,
) -> Element {
    let base = use_context::<ApiBase>().0;
    let mut preferences = use_context::<super::SharedPreferences>();
    let mut dialog_epoch = use_signal(|| 0_u64);
    let number_launch_chord = TEMPLATE_SHORTCUTS.number_chord("number");
    let quick_launch_chord = TEMPLATE_SHORTCUTS
        .bolt
        .label(TEMPLATE_SHORTCUTS.mac_desktop);
    let sequence_chord = TEMPLATE_SHORTCUTS
        .prefix
        .label(TEMPLATE_SHORTCUTS.mac_desktop);
    let list_base = base.clone();
    let mut templates = use_resource(move || {
        let base = list_base.clone();
        async move { crate::api::fetch_templates(&base).await }
    });
    let catalog_base = base.clone();
    let catalog = use_resource(move || {
        let base = catalog_base.clone();
        async move { crate::api::fetch_launch_catalog(&base).await }
    });
    let initial_draft = initial_template
        .as_ref()
        .map(Draft::from_template)
        .unwrap_or_default();
    let initial_baseline = initial_draft.clone();
    let mut draft = use_signal(move || initial_draft);
    let mut baseline = use_signal(move || initial_baseline);
    let mut previous = use_signal(|| initial_template.as_ref().map(|t| t.name.clone()));
    let opened = use_signal(|| initial_template.is_some());
    let mut leave_prompt = use_confirm_slot::<Departure, ()>();
    let mut error = use_signal(|| None::<String>);
    let mut busy = use_signal(|| false);
    let mut add_open = use_signal(|| false);
    let mut undo = use_signal(|| None::<Undo>);
    let mut undo_generation = use_signal(|| 0_u64);
    let binding_base = base.clone();
    let edit_binding: EventHandler<BindingEdit> = EventHandler::new(move |edit| {
        if busy() {
            return;
        }
        busy.set(true);
        error.set(None);
        let base = binding_base.clone();
        spawn(async move {
            match crate::api::fetch_preferences(&base).await {
                Ok(fresh) => {
                    let read_mark = fresh.quick_launch_template.clone();
                    let read_keys = fresh.template_shortcuts.clone();
                    apply_template_bindings(
                        &mut preferences.0.write(),
                        crate::api::template_bindings_after_read(fresh),
                    );
                    let mut current = preferences.0.write();
                    let value = match edit {
                        BindingEdit::Bolt(name, mark) => {
                            // Already as asked (an agent marked it, or moved the mark away): leave it.
                            let owned = current.quick_launch_template.as_ref().map(PeerText::raw)
                                == Some(name.as_str());
                            (mark != owned).then(|| {
                                let next = mark.then_some(name.into());
                                current.quick_launch_template = next.clone();
                                PreferenceValue::QuickLaunchTemplate(next)
                            })
                        }
                        BindingEdit::Number(name, key) => {
                            bind_number(&mut current.template_shortcuts, &name, key.as_deref());
                            Some(PreferenceValue::TemplateShortcuts(
                                current.template_shortcuts.clone(),
                            ))
                        }
                    };
                    drop(current);
                    // Keep edits and departure disabled through reconciliation,
                    // so neither can use a binding whose save just failed.
                    // A false acknowledgment also covers a lost reply after
                    // the helm committed. Discard the optimistic binding,
                    // then read again; neither the old nor attempted value
                    // tells us what the helm now holds. Keep the old
                    // observation only if that read fails too, and say so.
                    if let Some(value) = value {
                        let number_key = matches!(&value, PreferenceValue::TemplateShortcuts(_));
                        if !crate::api::write_template_binding(&base, value).await {
                            if number_key {
                                preferences.0.write().template_shortcuts = read_keys;
                            } else {
                                preferences.0.write().quick_launch_template = read_mark;
                            }
                            let detail = match crate::api::fetch_preferences(&base).await {
                                Ok(fresh) => {
                                    apply_template_bindings(
                                        &mut preferences.0.write(),
                                        crate::api::template_bindings_after_read(fresh),
                                    );
                                    if number_key {
                                        "showing the keys read from Farhelm"
                                    } else {
                                        "showing the mark read from Farhelm"
                                    }
                                }
                                Err(_) if number_key => "current keys could not be read",
                                Err(_) => "current mark could not be read",
                            };
                            let binding = if number_key {
                                "number key"
                            } else {
                                "quick-launch mark"
                            };
                            error.set(Some(format!(
                                "The {binding} may not have been saved; {detail}"
                            )));
                        }
                    }
                }
                Err(message) => error.set(Some(message)),
            }
            busy.set(false);
        });
    });
    use_effect(move || {
        let writing = busy();
        let loading = templates.pending();
        if !writing && !loading {
            restore_template_focus();
        }
    });
    let current = draft();
    // Escapes are presentation only: an unrelated save must retain the raw
    // name, including stored emoji joiners. Replacement is an explicit edit.
    let name_read_only = current
        .name
        .chars()
        .any(farhelm_proto::text::is_presentation_unsafe);
    let shown_name = if name_read_only {
        display_peer(&current.name)
    } else {
        current.name.clone()
    };
    // Field editors own raw text that can be invalid while being repaired.
    // Changing the approved source template must remount them rather than
    // letting that text describe a different saved vector. Renaming a draft
    // keeps this source identity stable, so it does not erase ongoing edits.
    let editor_source = previous().unwrap_or_default();
    let dirty = opened() && current != baseline();
    let catalog_rows = catalog
        .read()
        .as_ref()
        .and_then(|r| r.as_ref().ok())
        .cloned();
    let list = templates.read().clone();
    // Each kind has its own mounted control, so native listener updates
    // cannot change the promise of an element still painted on screen.
    let delete_kind = DeleteKind::of(
        list.as_ref()
            .and_then(|r| r.as_ref().ok())
            .map(Vec::as_slice),
        previous().as_deref(),
    );
    let names: Vec<String> = list
        .as_ref()
        .and_then(|r| r.as_ref().ok())
        .map(|rows| rows.iter().map(|t| t.name.clone()).collect())
        .unwrap_or_default();
    let departure_names = names.clone();
    let depart: EventHandler<Departure> = EventHandler::new(move |action| {
        if busy() {
            return;
        }
        // Rows retain a stored snapshot while an editor holds newer values.
        // Reselecting the source must not discard edits or reopen a deleted name.
        if let Departure::Open(template) = &action
            && previous().as_deref() == Some(template.name.as_str())
        {
            return;
        }
        add_open.set(false);
        if opened() && draft() != baseline() {
            // A new opening even when the prompt is already up (Escape or a
            // row click while it shows): answers drawn for the earlier
            // showing stop matching, so none of them can act on a departure
            // the prompt has since swapped out.
            leave_prompt.open(action, ());
        } else {
            leave_editor(
                action,
                draft,
                baseline,
                previous,
                opened,
                leave_prompt,
                error,
                departure_names.clone(),
                on_close,
            );
        }
    });

    let save_base = base.clone();
    // `after` is the leave prompt's token when the click is that prompt's
    // "save", and `None` for the editor's own save button.
    let save: EventHandler<Option<ConfirmToken<Departure>>> = EventHandler::new(move |after| {
        if busy() {
            return;
        }
        // A stale prompt answer refuses before validation, so it neither
        // saves nor puts an error on the prompt now on screen.
        let after = match after {
            None => None,
            Some(shown) => match answer_save_and_leave(&leave_prompt, &shown) {
                Some(action) => Some(action),
                None => return,
            },
        };
        let value = draft();
        let catalog_value = catalog.peek();
        let catalog_rows = catalog_value
            .as_ref()
            .and_then(|r| r.as_ref().ok())
            .map(Vec::as_slice);
        // Match the API's shape refusals locally, especially the empty name:
        // its PUT path cannot otherwise reach the template handler at all.
        let shape_refusal = check_template_shape(&LaunchTemplate {
            name: value.name.clone(),
            fields: value.fields.clone(),
        })
        .err()
        .map(crate::local_validation_notice);
        if let Some(message) = shape_refusal
            .or_else(|| value.refusal(catalog_rows))
            .or_else(|| name_refusal(templates, previous().as_deref(), &value.name))
        {
            error.set(Some(message));
            return;
        }
        let old = previous();
        let base = save_base.clone();
        busy.set(true);
        error.set(None);
        // A new name or a rename re-reads the names just before writing: the
        // list this dialog holds was read when it opened or after its own last
        // write, so it cannot see a template an agent or the command line
        // created since, and the write would replace it (SPEC_impl.md,
        // "Templates"). As for Undo, the read is declared before
        // the save's ticket so it does not wait for the save it checks.
        let listed = (old.as_deref() != Some(value.name.as_str()))
            .then(|| crate::api::fetch_templates(&base));
        let saving_request = crate::api::put_template(&base, &value.name, &value.fields);
        spawn(async move {
            let refusal = match listed {
                None => None,
                Some(listed) => match listed.await {
                    Ok(rows) => {
                        let names = rows.iter().map(|t| t.name.as_str()).collect();
                        save_name_refusal(None, &value.name, &ListedNames::Loaded(names))
                    }
                    Err(message) => Some(format!(
                        "Cannot check that no other template has this name: {message}"
                    )),
                },
            };
            let result = match refusal {
                Some(message) => Err(message),
                None => saving_request.await,
            };
            let result = match (result, old) {
                (Ok(()), Some(old)) if old != value.name => {
                    // Another window may have marked this name, or moved the
                    // mark away, since this dialog read the bindings. Refresh
                    // both fields, keeping a write of ours still in flight.
                    // Rename remains ordinary last-writer-wins writes across
                    // clients.
                    let fetched = match crate::api::fetch_preferences(&base).await {
                        Ok(fetched) => fetched,
                        Err(message) => {
                            busy.set(false);
                            error.set(Some(format!("The new name was saved, but shortcut bindings could not be read; the old template was kept: {message}")));
                            templates.restart();
                            return;
                        }
                    };
                    let fetched = crate::api::template_bindings_after_read(fetched);
                    preferences.0.write().quick_launch_template = fetched.quick_launch_template;
                    preferences.0.write().template_shortcuts = fetched.template_shortcuts;
                    // Repoint before deletion: the shared delete path clears
                    // only an old-name mark, and cannot clear the new one.
                    if preferences
                        .0
                        .peek()
                        .quick_launch_template
                        .as_ref()
                        .map(PeerText::raw)
                        == Some(old.as_str())
                    {
                        preferences.0.write().quick_launch_template =
                            Some(value.name.clone().into());
                        if !crate::api::write_template_binding(
                            &base,
                            crate::api::PreferenceValue::QuickLaunchTemplate(Some(
                                value.name.clone().into(),
                            )),
                        )
                        .await
                        {
                            // The failed mark is discarded; show the helm's again.
                            preferences.0.write().quick_launch_template = Some(old.clone().into());
                            busy.set(false);
                            error.set(Some("The new name was saved, but the quick launch mark could not be moved; the old template was kept".into()));
                            templates.restart();
                            return;
                        }
                    }
                    let mut bindings = preferences.0.peek().template_shortcuts.clone();
                    let before = bindings.clone();
                    rename_number_binding(&mut bindings, &old, &value.name);
                    if bindings != before {
                        preferences.0.write().template_shortcuts = bindings.clone();
                        if !crate::api::write_template_binding(
                            &base,
                            crate::api::PreferenceValue::TemplateShortcuts(bindings),
                        )
                        .await
                        {
                            // The failed map is discarded; show the helm's keys again.
                            preferences.0.write().template_shortcuts = before;
                            busy.set(false);
                            error.set(Some("The new name was saved, but its number key could not be moved; the old template was kept".into()));
                            templates.restart();
                            return;
                        }
                    }
                    let deleting_request = crate::api::delete_template(&base, &old);
                    if !crate::api::prepare_template_delete().await {
                        busy.set(false);
                        error.set(Some("The new name was saved, but a shortcut write was interrupted; the old template was kept".into()));
                        templates.restart();
                        return;
                    }
                    deleting_request
                        .await
                        .map_err(crate::api::SubmissionError::into_text)
                }
                (result, _) => result,
            };
            busy.set(false);
            match result {
                Ok(()) => {
                    baseline.set(value.clone());
                    previous.set(Some(value.name.clone()));
                    let mut names = templates
                        .peek()
                        .as_ref()
                        .and_then(|r| r.as_ref().ok())
                        .map(|rows| rows.iter().map(|t| t.name.clone()).collect::<Vec<_>>())
                        .unwrap_or_default();
                    if !names.contains(&value.name) {
                        names.push(value.name.clone());
                    }
                    templates.restart();
                    if let Some(action) = after {
                        leave_editor(
                            action,
                            draft,
                            baseline,
                            previous,
                            opened,
                            leave_prompt,
                            error,
                            names,
                            on_close,
                        );
                    } else {
                        leave_prompt.clear();
                    }
                }
                Err(message) => error.set(Some(message)),
            }
        });
    });

    let delete_base = base.clone();
    let delete = move |shown_kind| {
        // Save's authoritative reload must land before Delete snapshots the
        // fields for Undo; the retained resource value can still be older.
        if busy() || templates.pending() {
            return;
        }
        let Some(name) = previous() else {
            return;
        };
        let Some(template) = templates
            .peek()
            .as_ref()
            .and_then(|r| r.as_ref().ok())
            .and_then(|rows| rows.iter().find(|t| t.name == name))
            .cloned()
        else {
            error.set(Some(
                "Reload the template list before deleting this template".into(),
            ));
            return;
        };
        let base = delete_base.clone();
        busy.set(true);
        error.set(None);
        let deleting_request = crate::api::delete_template(&base, &name);
        spawn(async move {
            if !crate::api::prepare_template_delete().await {
                busy.set(false);
                error.set(Some(
                    "A shortcut write was interrupted; try deleting the template again".into(),
                ));
                return;
            }
            match deleting_request.await {
                Ok(()) => {
                    if preferences
                        .0
                        .peek()
                        .quick_launch_template
                        .as_ref()
                        .map(PeerText::raw)
                        == Some(name.as_str())
                    {
                        preferences.0.write().quick_launch_template = None;
                    }
                    preferences
                        .0
                        .write()
                        .template_shortcuts
                        .retain(|_, owner| owner.raw() != name);
                    undo_generation += 1;
                    let generation = undo_generation();
                    // Whether Undo follows is the kind the painted control
                    // promised (its fixed `shown_kind`), not
                    // a fresh shape check of the value deleted: a list reload
                    // landing between the render the user saw and the click
                    // must not turn "delete" into a delete with no Undo, or
                    // "delete permanently" into one that offers it.
                    undo.set(undo_after_delete(shown_kind, template, generation));
                    leave_editor(
                        Departure::Back,
                        draft,
                        baseline,
                        previous,
                        opened,
                        leave_prompt,
                        error,
                        Vec::new(),
                        on_close,
                    );
                    templates.restart();
                    // This is the user-visible undo window, not a readiness
                    // wait. Dioxus cancels the task when this dialog unmounts.
                    spawn(async move {
                        crate::reader::sleep_ms(10_000).await;
                        if undo
                            .peek()
                            .as_ref()
                            .is_some_and(|v| v.generation == generation)
                        {
                            undo.set(None);
                        }
                    });
                }
                Err(crate::api::SubmissionError::Refused(message)) => error.set(Some(message)),
                Err(crate::api::SubmissionError::Ambiguous(_)) => {
                    // The helm may have committed this delete. A fresh list
                    // resolves what is present; no reply justifies Undo.
                    undo.set(None);
                    error.set(Some(
                        "The template may have been deleted; reloading the template list".into(),
                    ));
                    templates.restart();
                }
            }
            busy.set(false);
        });
    };
    let undo_base = base.clone();
    let restore = move |_| {
        if busy() {
            return;
        }
        let Some(mut value) = undo() else {
            return;
        };
        // Undo is a new save too; never restore a retired launch choice.
        value.template.fields.workspace_trust = None;
        // Re-read names at undo time; the list after deletion alone cannot
        // see a name another client took during the ten-second window.
        let base = undo_base.clone();
        busy.set(true);
        error.set(None);
        // Validation belongs before this save's ticket. Both futures are
        // declared now, so later requests still see the pending restoration.
        let listed = crate::api::fetch_templates(&base);
        let saving_request =
            crate::api::put_template(&base, &value.template.name, &value.template.fields);
        spawn(async move {
            let result = match listed.await {
                Ok(rows) => {
                    let names = ListedNames::Loaded(rows.iter().map(|t| t.name.as_str()).collect());
                    match save_name_refusal(None, &value.template.name, &names) {
                        Some(message) => Err(message),
                        None => saving_request.await,
                    }
                }
                Err(message) => Err(format!(
                    "Cannot check the deleted template's name: {message}"
                )),
            };
            busy.set(false);
            match result {
                Ok(()) => {
                    undo.set(None);
                    templates.restart();
                }
                Err(message) => error.set(Some(message)),
            }
        });
    };

    let disabled = busy();
    // The leave prompt's opening, read once here and moved into each of its
    // buttons below, so every answer names the showing it was drawn for.
    let prompt = leave_prompt.current();
    let inferred =
        previous().is_some() && baseline().fields.kind.is_none() && current.fields.kind.is_some();
    // The same resolved mark and number label drive both visible controls and
    // each menu's movement admission. Resource pending/error semantics remain
    // in the existing list match below; this does not introduce another read.
    let rendered_rows = {
        let preferences = preferences.0.read();
        list.as_ref()
            .and_then(|result| result.as_ref().ok())
            .map(|rows| {
                template_list_rows(
                    rows,
                    preferences
                        .quick_launch_template
                        .as_ref()
                        .map(PeerText::raw),
                    &preferences.template_shortcuts,
                )
            })
            .unwrap_or_default()
    };
    rsx! {
        div { class: "host-settings-backdrop", role: "presentation",
            // The capped dialog can move while keeping its own size. Its
            // full-window backdrop also observes width-only viewport changes.
            onresize: move |_| dialog_epoch += 1,
            ModalDialog { class: if opened() { "templates-dialog templates-editor-open" } else { "templates-dialog" }, role: "dialog", aria_modal: "true", aria_label: "Templates",
                handle: DialogHandle::new(DIALOG_SELECTOR),
                // The existing dialog itself scrolls. Busy entry must not add
                // a nested region or replace its dialog role and name.
                root_scroll: template_recovery(),
                focus: DialogFocus::remembered(".templates-new", Some(".templates-close"), false, None),
                onscroll: move |_| dialog_epoch += 1,
                onresize: move |_| dialog_epoch += 1,
                on_escape: move |event: KeyboardEvent| {
                    if !event.is_composing() {
                        event.stop_propagation();
                        if add_open() { add_open.set(false); focus_template_control(".templates-add"); }
                        else { depart.call(Departure::Close); }
                    }
                },
                DialogHeader {
                    class: "templates-header",
                    title: rsx! { "Templates" },
                    summary: rsx! { "Saved edits to combine in the session launcher" },
                    tile: rsx! { crate::icons::ActionIcon { glyph: crate::icons::ActionGlyph::Templates } },
                    close: rsx! {
                        button { r#type: "button", class: "dialog-close templates-close", disabled,
                            aria_label: "Close templates", "data-tooltip": "Close templates; unsaved changes ask before leaving",
                            onclick: move |_| depart.call(Departure::Close), "×" }
                    },
                }
                // This is content padding, not another scroll owner. The root's
                // busy keyboard-entry contract covers both responsive views.
                div { class: "templates-content",
                p { class: "host-settings-help templates-help",
                    "A template is a named set of launcher edits. In New session, type tl:name to apply one; it changes only the fields it sets. Templates stack, the later one winning: tl:my-claude then tl:webbuilder starts Claude in a managed checkout of acme/web on build-box. ⚡ marks the template {quick_launch_chord} launches directly; a number key launches its template with {number_launch_chord}. A number key can hold a partial template; press {sequence_chord} and then several keys to combine them, Enter to launch."
                }
                if quick_launch_hint {
                    p { class: "templates-quick-hint", role: "status", "Mark a template with ⚡ to launch it directly with {quick_launch_chord}." }
                }
                if number_shortcut_hint {
                    p { class: "templates-number-hint", role: "status", "Give a template a number key with the key button beside its ⚡; {number_launch_chord} launches it directly." }
                }
                div { class: "templates-panes",
                    aside { class: "templates-sidebar", aria_label: "Template list", aria_busy: templates.pending(),
                        button { r#type: "button", class: "btn btn-neutral templates-new", disabled, "data-tooltip": "Start a new template; unsaved changes ask before leaving", onclick: move |_| depart.call(Departure::New), "+ New template" }
                        match list.clone() {
                            None => rsx! { p { class: "templates-hint", "Loading templates" } },
                            Some(Err(_)) => rsx! {},
                            Some(Ok(_)) => rsx! {
                                ul { class: "templates-list",
                                    for row in rendered_rows {
                                        li { key: "{row.template.name}",
                                            button {
                                                r#type: "button", class: "templates-quick-mark",
                                                // A lit bolt stays clickable even when its template has since
                                                // become ineligible (edited through the API, say): clicking
                                                // the lit bolt is how a mark is cleared, and a disabled one
                                                // would leave an unusable mark with no way to turn it off.
                                                disabled: disabled || templates.pending() || (!quick_launch_eligible(&row.template.fields) && !row.anchor.marked),
                                                aria_label: "Quick launch {display_peer(&row.template.name)}",
                                                aria_pressed: row.anchor.marked,
                                                "data-tooltip": if row.anchor.marked { format!("{quick_launch_chord} starts a session from this template right away, without opening New. Only one template can hold ⚡. Click to turn off.") } else if !quick_launch_eligible(&row.template.fields) { format!("{quick_launch_chord} starts a session from the ⚡ template right away, without opening New. Only one template can hold ⚡. Can't mark this one: an agent launch needs an agent; a command launch needs a nonblank command.") } else { format!("{quick_launch_chord} starts a session from the ⚡ template right away, without opening New. Only one template can hold ⚡. Click to mark this one.") },
                                                onclick: { let name = row.template.name.clone(); move |_| { let lit = preferences.0.read().quick_launch_template.as_ref().map(PeerText::raw) == Some(name.as_str()); edit_binding.call(BindingEdit::Bolt(name.clone(), !lit)) } },
                                                "⚡"
                                            }
                                            NumberShortcutMenu {
                                                key: "number-{row.template.name}", name: row.template.name.clone(),
                                                anchor: row.anchor.clone(),
                                                launchable: quick_launch_eligible(&row.template.fields), disabled: disabled || templates.pending(),
                                                bindings: preferences.0.read().template_shortcuts.clone(),
                                                layout_epoch, dialog_epoch,
                                                on_choose: { let name = row.template.name.clone(); move |key| edit_binding.call(BindingEdit::Number(name.clone(), key)) },
                                            }
                                            button { r#type: "button", class: "templates-row", disabled: disabled || templates.pending(), aria_pressed: previous().as_ref()==Some(&row.template.name), "data-tooltip": "Edit template {display_peer(&row.template.name)}", onclick: move |_| depart.call(Departure::Open(Box::new(row.template.clone()))),
                                                span { class: "templates-row-name peer-value", dir: "ltr", "{display_peer(&row.template.name)}" }
                                                TemplateSummary { class: "templates-row-summary", fields: row.template.fields.clone() }
                                                if row.anchor.marked { span { class: "templates-quick-chord font-mono", "{quick_launch_chord}" } }
                                            }
                                        }
                                    }
                                }
                            },
                        }
                        // A failed or absent read is unknown, not an empty list.
                        if let Some(Ok(rows)) = &list {
                            p { class: "templates-count", "{rows.len()} " if rows.len() == 1 { "template" } else { "templates" } }
                        }
                    }
                    section { class: "templates-editor", aria_label: "Template editor",
                        if opened() {
                            button { r#type: "button", class: "templates-back", disabled, "data-tooltip": "Back to templates; unsaved changes ask before leaving", onclick: move |_| depart.call(Departure::Back), "‹ Templates" }
                            div { class: "templates-editor-heading",
                                h3 { if previous().is_some() { "Edit template" } else { "New template" } }
                                if dirty { span { class: "templates-unsaved", "Unsaved changes" } }
                            }
                            label { class: "templates-field",
                                span { "Name" }
                                // The name is a literal tl: search key; browser
                                // correction would change what the user chose.
                                input { class: "templates-name", value: shown_name, readonly: name_read_only, disabled, placeholder: "my-claude",
                                    autocomplete: "off", autocorrect: "off", autocapitalize: "none", spellcheck: "false",
                                    "data-tooltip": "Template name: use tl:name to apply it in New session", oninput: move |e| { if !name_read_only { draft.with_mut(|d| d.name=e.value()); } } }
                            }
                            if name_read_only {
                                p { class: "templates-hint", "This name contains invisible formatting, shown as escapes. It stays unchanged until replaced." }
                                button { r#type: "button", class: "btn btn-neutral templates-replace-name", disabled,
                                    onclick: move |_| { draft.with_mut(|d| d.name.clear()); focus_template_control(".templates-name"); }, "Replace name" }
                            }
                            p { class: "templates-quick-state",
                                if previous().is_some() && preferences.0.read().quick_launch_template.as_ref().map(PeerText::raw) == previous().as_deref() {
                                    "⚡ Quick launch: {quick_launch_chord} starts a session from this saved template."
                                } else if quick_launch_eligible(&current.fields) {
                                    "Not the quick launch. The ⚡ in the list makes it one."
                                } else {
                                    "Can't be the quick launch: an agent launch needs an agent; a command launch needs a nonblank command."
                                }
                            }
                            div { class: "templates-field",
                                span { "Switches launcher to" }
                                div { class: "segmented-control", role: "group", aria_label: "Switches launcher to",
                                    for (kind,text) in [(Some(LauncherKind::Agent),"Agent"),(Some(LauncherKind::Command),"Command"),(None,"Don't switch")] {
                                        button { r#type: "button", disabled, aria_pressed: current.fields.kind==kind, "data-tooltip": "Switches launcher to: {text}; incompatible fields stay visible", onclick: move |_| { draft.with_mut(|d| d.fields.kind=kind); add_open.set(false); }, "{text}" }
                                    }
                                }
                            }
                            if inferred { p { class: "templates-hint templates-inferred", "This stored template does not switch the launcher yet. Saving will store the switch inferred from its fields." } }
                            h4 { class: "templates-sets-heading", "Sets" }
                            for field in Field::ALL.into_iter().filter(|f| f.is_set(&current.fields)) {
                                FieldEditor { key: "{editor_source}-{field.key()}", field, draft, hosts: hosts.clone(), catalog: catalog_rows.clone(), disabled }
                            }
                            div { class: "templates-add-area",
                                button { r#type: "button", class: "btn btn-neutral templates-add", disabled, aria_expanded: add_open(), "data-tooltip": "Add a field this template will set; fields not shown are left as is", onclick: move |_| add_open.toggle(), "+ Add field" }
                                if add_open() {
                                    div { class: "templates-add-menu", role: "group", aria_label: "Add field",
                                        for field in Field::ALL.into_iter().filter(|f| !f.is_set(&current.fields) && f.offered(&current.fields)) {
                                            button { r#type: "button", disabled, "data-tooltip": "Add {field.label().to_ascii_lowercase()} to the template", onclick: move |_| { draft.with_mut(|d| field.add(&mut d.fields)); add_open.set(false); }, "{field.label()}" }
                                        }
                                    }
                                }
                            }
                            p { class: "templates-hint", "Fields not shown are left as is. Another template can supply choices this one leaves out." }
                            DialogFooter { class: "templates-editor-footer",
                                if let Some(shown)=prompt.clone() {
                                    div { class: "templates-departure", role: "status",
                                        // Reveal the footer's padding too: focusing only the answer
                                        // leaves Save against the phone scrollport's bottom edge.
                                        onmounted: move |_| { document::eval("requestAnimationFrame(() => { const prompt = document.querySelector('.templates-departure'); prompt?.closest('.templates-editor-footer')?.scrollIntoView({block: 'end'}); prompt?.querySelector('.templates-save-leave')?.focus({preventScroll: true}); })"); },
                                        p { "Save these changes before leaving?" }
                                        DialogActions { class: "templates-actions",
                                            button { r#type: "button", class: "btn btn-neutral templates-keep-editing", disabled, "data-tooltip": "Stay here with the unsaved draft", onclick: { let shown=shown.clone(); move |_| { if answer_keep_editing(&mut leave_prompt,&shown,busy()) { focus_template_control(".templates-name"); } } }, "Keep editing" }
                                            button { r#type: "button", class: "btn btn-neutral templates-discard", disabled, "data-tooltip": "Discard these changes, then continue", onclick: { let names=names.clone(); let shown=shown.clone(); move |_| { if let Some(action)=answer_discard(&mut leave_prompt,&shown,busy()) { leave_editor(action,draft,baseline,previous,opened,leave_prompt,error,names.clone(),on_close) } } }, "Discard" }
                                            button { r#type: "button", class: "btn btn-primary templates-save-leave", disabled, "data-tooltip": "Save these changes, then continue", onclick: move |_| save.call(Some(shown.clone())), "Save" }
                                        }
                                    }
                                } else {
                                    DialogActions { class: "templates-actions",
                                        if previous().is_some() {
                                            button { r#type: "button", class: "btn btn-neutral templates-duplicate", disabled: disabled || templates.pending(), "data-tooltip": "Open a new unsaved copy with a free name", onclick: move |_| depart.call(Departure::Duplicate), "Duplicate" }
                                            TemplateDeleteButtons { kind: delete_kind, disabled: disabled || templates.pending(), on_delete: delete }
                                        }
                                        button { r#type: "button", class: "btn btn-primary templates-save", disabled, "data-tooltip": "Save the template's name, switch and shown fields", onclick: move |_| save.call(None), "Save" }
                                    }
                                }
                            }
                        } else { p { class: "templates-empty", "Choose a template to edit, or make a new one." } }
                    }
                }
                // Recovery belongs to both responsive views: the sidebar is
                // hidden while editing on a phone, but its name guard still applies.
                if let Some(Err(message)) = list {
                    div { class: "templates-list-error",
                        p { class: "templates-error", "Templates unavailable: {message}" }
                        button { r#type: "button", class: "btn btn-neutral templates-retry", disabled, "data-tooltip": "Load the template list again without discarding this draft", onclick: move |_| templates.restart(), "Retry" }
                    }
                }
                if let Some(message)=error() { p { class: "templates-error", role: "status", "{message}" } }
                if let Some(value)=undo() {
                    div { class: "templates-undo-notice", role: "status",
                        span { "Deleted {display_peer(&value.template.name)}." }
                        button { r#type: "button", class: "btn btn-neutral templates-undo", disabled, "data-tooltip": "Restore the deleted template if its old name is still free", onclick: restore, "Undo" }
                    }
                }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    /// Keep projection tests independent of catalog lookup and launcher rules.
    /// Empty fields still render the nonempty one-line summary used by real rows.
    fn movement_templates(names: &[&str]) -> Vec<LaunchTemplate> {
        names
            .iter()
            .map(|name| LaunchTemplate {
                name: (*name).into(),
                fields: TemplateFields::default(),
            })
            .collect()
    }

    /// A target's key must describe its own centered line and its prefix height,
    /// while preserving harmless equal-height permutations elsewhere in the list.
    #[test]
    fn template_anchor_keys_follow_rendered_order_mark_and_number() {
        let rows = movement_templates(&["a", "b", "c", "d"]);
        let bindings = BTreeMap::from([("2".into(), PeerText::from("b"))]);
        let before = template_list_rows(&rows, Some("c"), &bindings);
        let after = template_list_rows(&rows, Some("a"), &bindings);
        assert_ne!(
            before[0].anchor, after[0].anchor,
            "own centered toggle moves"
        );
        assert_ne!(before[1].anchor, after[1].anchor, "the new line is above b");
        assert_ne!(before[2].anchor, after[2].anchor, "c loses its own line");
        assert_eq!(
            before[3].anchor, after[3].anchor,
            "balanced prefix is inert"
        );
        assert_eq!(after[1].anchor.bound.as_deref(), Some("2"));

        let reordered = movement_templates(&["c", "b", "a", "d"]);
        let permuted = template_list_rows(&reordered, Some("c"), &bindings);
        assert_ne!(
            before[1].anchor, permuted[1].anchor,
            "a marked row crosses b"
        );
        assert_eq!(
            before[3].anchor, permuted[3].anchor,
            "same prefix height at d"
        );
        let changed_binding = BTreeMap::from([("8".into(), PeerText::from("b"))]);
        let rebound = template_list_rows(&rows, Some("c"), &changed_binding);
        assert_ne!(
            before[1].anchor, rebound[1].anchor,
            "raw binding still updates props"
        );
        assert_eq!(
            before[1].anchor.movement_key(),
            rebound[1].anchor.movement_key(),
            "known digits retain chord width"
        );
        assert_eq!(
            before[0].anchor, rebound[0].anchor,
            "other labels are inert"
        );

        let renamed = movement_templates(&["b", "c", "d", "z"]);
        let renamed = template_list_rows(&renamed, Some("c"), &bindings);
        assert_ne!(
            before[1].anchor, renamed[0].anchor,
            "surviving b changes index"
        );
    }

    /// Tabular digit changes must not close a popup; unknown text must not
    /// acquire guessed Unicode widths or lose the formatter's fallback behavior.
    #[test]
    fn template_number_width_shape_groups_only_actual_known_digit_chords() {
        let digit_shape = template_number_width_shape(Some("0"));
        let mut labels = std::collections::HashSet::new();
        for key in TEMPLATE_SHORTCUTS.number.keys {
            labels.insert(TEMPLATE_SHORTCUTS.number_chord(key.value));
            assert_eq!(template_number_width_shape(Some(key.value)), digit_shape);
        }
        assert_eq!(labels.len(), 10, "all actual digit labels remain distinct");
        assert_eq!(
            TEMPLATE_SHORTCUTS.number_chord("Digit8"),
            TEMPLATE_SHORTCUTS.number_chord("8")
        );
        assert_eq!(
            template_number_width_shape(Some("Digit8")),
            digit_shape,
            "identical fallback rendering stays equivalent"
        );

        let unbound = TEMPLATE_SHORTCUTS.number_chord(" –");
        assert_eq!(template_number_width_shape(None), unbound);
        assert_ne!(template_number_width_shape(None), digit_shape);
        for unknown in [
            "x2",
            "x8",
            "二",
            "2\u{fe0f}\u{20e3}",
            "8\u{fe0f}\u{20e3}",
            "ab",
            "😀",
        ] {
            assert_eq!(
                template_number_width_shape(Some(unknown)),
                TEMPLATE_SHORTCUTS.number_chord(unknown)
            );
        }
        assert_ne!(
            template_number_width_shape(Some("x2")),
            template_number_width_shape(Some("x8")),
            "do not normalize digits inside arbitrary text"
        );
    }

    /// Only a changed movement shape captures an opening. Raw binding churn
    /// remains visible through strict props without replaying a synthetic cause.
    #[test]
    fn template_movement_ignores_digit_rebinding_but_retains_placeholder_changes() {
        let key = |bound: Option<&str>| TemplateAnchorKey {
            index: 0,
            marked_before: 0,
            marked: false,
            bound: bound.map(str::to_string),
        };
        let mut movement = TemplateAnchorMovement::new(key(Some("2")));
        movement.observe(&key(Some("8")), Some(1));
        assert!(
            !movement.take_for(Some(1)),
            "same-width known digit is inert"
        );
        movement.observe(&key(Some("Digit8")), Some(1));
        assert!(
            !movement.take_for(Some(1)),
            "same displayed fallback label is inert"
        );
        movement.observe(&key(None), Some(1));
        assert!(
            movement.take_for(Some(1)),
            "bound to unbound changes width shape"
        );
        movement.observe(&key(Some("0")), Some(1));
        assert!(
            movement.take_for(Some(1)),
            "unbound to bound changes width shape"
        );
    }

    /// Callback replacement must not forget a committed intermediate layout.
    /// A fresh same-row opening, however, measures after the old cause and is safe.
    #[test]
    fn template_movement_retains_a_b_a_and_rejects_a_new_opening() {
        let rows = movement_templates(&["a"]);
        let a = template_list_rows(&rows, None, &BTreeMap::new())
            .remove(0)
            .anchor;
        let b = template_list_rows(&rows, Some("a"), &BTreeMap::new())
            .remove(0)
            .anchor;
        let mut movement = TemplateAnchorMovement::new(a.clone());
        movement.observe(&a, Some(1));
        assert!(
            !movement.take_for(Some(1)),
            "first/same render is a baseline"
        );
        movement.observe(&b, Some(1));
        movement.observe(&a, Some(1));
        assert!(
            movement.take_for(Some(1)),
            "A→B→A still invalidated opening 1"
        );
        assert!(!movement.take_for(Some(1)), "delivery is drained once");
        movement.observe(&b, Some(1));
        assert!(
            !movement.take_for(Some(2)),
            "same-row reopen rejects old cause"
        );
        movement.observe(&a, Some(2));
        assert!(
            movement.take_for(Some(2)),
            "a later change can invalidate reopening"
        );
        movement.observe(&b, None);
        assert!(!movement.take_for(Some(3)), "closed changes never replay");
    }

    /// Own the actual local adapter and registration under a controllable VDOM.
    /// Inputs are cells so tests can request consecutive synchronous renders
    /// without polling effects between them or touching process environment.
    #[derive(PartialEq)]
    struct TemplateMovementFixture {
        index: Cell<usize>,
        mounted: Cell<bool>,
        scope: Cell<Option<ScopeId>>,
        controls: RefCell<Option<(Signal<bool>, Signal<u64>)>>,
        closes: Cell<usize>,
    }

    impl TemplateMovementFixture {
        /// Start open before the initial effect to expose baseline mistakes.
        fn new() -> Rc<Self> {
            Rc::new(Self {
                index: Cell::new(0),
                mounted: Cell::new(true),
                scope: Cell::new(None),
                controls: RefCell::new(None),
                closes: Cell::new(0),
            })
        }

        /// Publish mutations while intentionally leaving the post-render effect
        /// unsettled. This is the boundary the race tests exist to exercise.
        fn render_index(self: &Rc<Self>, dom: &mut VirtualDom, index: usize) {
            self.index.set(index);
            dom.mark_dirty(self.scope.get().expect("fixture anchor mounted"));
            dom.render_immediate_to_vec();
        }

        /// Mirror the existing opener's synchronous generation increment.
        /// No layout effect is allowed to treat this as the preceding opening.
        fn reopen(&self, dom: &VirtualDom) {
            let (mut open, mut generation) = self.controls.borrow().expect("fixture controls");
            dom.in_runtime(|| {
                generation += 1;
                open.set(true);
            });
        }
    }

    /// Keep the adapter in its own scope so removal exercises real cleanup.
    #[component]
    fn TemplateMovementTestAnchor(fixture: Rc<TemplateMovementFixture>) -> Element {
        let open = use_signal(|| true);
        let generation = use_signal(|| 1_u64);
        fixture.scope.set(Some(dioxus::core::current_scope_id()));
        fixture.controls.replace(Some((open, generation)));
        let closed = fixture.clone();
        let registration = use_registration(
            use_context::<MovementOwner>(),
            Interest::new(AnchorRegion::Templates),
            move || (*open.peek()).then(|| "fixture".into()),
            move || {
                closed.closes.set(closed.closes.get() + 1);
                let mut slot = open;
                slot.set(false);
            },
        );
        let anchor = TemplateAnchorKey {
            index: fixture.index.get(),
            marked_before: 0,
            marked: false,
            bound: None,
        };
        use_template_anchor_movement(&anchor, open, generation, registration);
        rsx! { span { "{anchor.index}" } }
    }

    /// The context remains mounted when the anchor scope is deliberately removed.
    fn template_movement_test_app(fixture: Rc<TemplateMovementFixture>) -> Element {
        use_context_provider(MovementOwner::default);
        rsx! {
            if fixture.mounted.get() {
                TemplateMovementTestAnchor { fixture }
            }
        }
    }

    /// Exercise the production hook under Dioxus's real render/effect scheduler.
    /// A→B→A is two published mutation batches before the deduplicated continuation.
    #[test]
    fn template_movement_component_keeps_intermediate_cause_after_callback_replacement() {
        let fixture = TemplateMovementFixture::new();
        let mut dom = VirtualDom::new_with_props(template_movement_test_app, fixture.clone());
        dom.rebuild_to_vec();
        dom.process_events();
        assert_eq!(fixture.closes.get(), 0, "initial effect must not close");
        fixture.render_index(&mut dom, 0);
        dom.process_events();
        assert_eq!(fixture.closes.get(), 0, "same key is inert");
        fixture.render_index(&mut dom, 1);
        fixture.render_index(&mut dom, 0);
        assert_eq!(fixture.closes.get(), 0, "continuation is still pending");
        dom.process_events();
        assert_eq!(
            fixture.closes.get(),
            1,
            "intermediate change survives A→B→A"
        );
    }

    /// A same-row reopening before delivery must reject the earlier capture.
    /// The generation also covers an in-flight placement measurement: this
    /// adapter must not wait for a DOM rectangle before invalidating an opening.
    #[test]
    fn template_movement_component_does_not_replay_into_same_row_reopen() {
        let fixture = TemplateMovementFixture::new();
        let mut dom = VirtualDom::new_with_props(template_movement_test_app, fixture.clone());
        dom.rebuild_to_vec();
        dom.process_events();
        fixture.render_index(&mut dom, 1);
        fixture.reopen(&dom);
        dom.process_events();
        assert_eq!(fixture.closes.get(), 0);
        fixture.render_index(&mut dom, 2);
        dom.process_events();
        assert_eq!(fixture.closes.get(), 1, "new cause reaches the new opening");
    }

    /// Queued work belongs to the old scope and cannot reach a replacement.
    /// Removal uses the framework's normal queued-effect and registration cleanup.
    #[test]
    fn template_movement_component_drops_pending_delivery_with_scope() {
        let fixture = TemplateMovementFixture::new();
        let mut dom = VirtualDom::new_with_props(template_movement_test_app, fixture.clone());
        dom.rebuild_to_vec();
        dom.process_events();
        fixture.render_index(&mut dom, 1);
        fixture.mounted.set(false);
        dom.mark_dirty(ScopeId::APP);
        dom.render_immediate_to_vec();
        dom.process_events();
        assert_eq!(
            fixture.closes.get(),
            0,
            "dead scope's pending effect is removed"
        );
        fixture.mounted.set(true);
        dom.mark_dirty(ScopeId::APP);
        dom.render_immediate_to_vec();
        dom.process_events();
        assert_eq!(
            fixture.closes.get(),
            0,
            "replacement mount starts a new baseline"
        );
    }

    /// Removing a name must remove its policy; re-adding it starts a new Base
    /// choice instead of resurrecting a hidden Fixed setting.
    #[test]
    fn name_field_presence_owns_its_kind() {
        let mut fields = TemplateFields {
            name: Some("review".into()),
            name_kind: Some(TemplateNameKind::Fixed),
            ..Default::default()
        };
        Field::Name.remove(&mut fields);
        assert_eq!(fields.name, None);
        assert_eq!(fields.name_kind, None);
        Field::Name.add(&mut fields);
        assert_eq!(fields.name.as_deref(), Some(""));
        assert_eq!(fields.name_kind, None);
    }

    /// The editor distinguishes an absent edit from a present reset. Prefix
    /// words are displayed as shell text, with unsafe persisted bytes escaped
    /// without changing the stored value.
    #[test]
    fn wrapper_field_keeps_reset_and_safe_display_distinct() {
        let mut fields = Draft::default().fields;
        assert!(Field::Wrapper.offered(&fields));
        assert!(!Field::Wrapper.is_set(&fields));
        Field::Wrapper.add(&mut fields);
        assert_eq!(fields.wrapper_prefix, Some(None));
        assert!(template_summary(&fields).contains("Wrapper prefix none"));
        fields.wrapper_prefix = Some(Some(vec!["wrapper".into(), "--".into()]));
        assert!(template_summary(&fields).contains("Wrapper prefix wrapper --"));
        fields.wrapper_prefix = Some(Some(vec!["wrapper\u{202e}".into()]));
        assert!(
            Field::Wrapper
                .escaped_read_only_value(&fields)
                .unwrap()
                .contains("<U+202E>")
        );
        Field::Wrapper.remove(&mut fields);
        assert_eq!(fields.wrapper_prefix, None);
        fields.kind = Some(LauncherKind::Command);
        assert!(!Field::Wrapper.offered(&fields));
    }

    /// Container edits distinguish absence from an explicit off reset. Partial
    /// templates need no agent or destination, but invalid configuration and
    /// command-tab use must refuse instead of silently dropping isolation.
    #[test]
    fn container_field_preserves_presence_and_refuses_invalid_choices() {
        let mut draft = Draft::default();
        assert!(!Field::Container.is_set(&draft.fields));
        Field::Container.add(&mut draft.fields);
        assert_eq!(draft.fields.container, Some(None));
        assert_eq!(draft.refusal(None), None);
        let config = farhelm_proto::ContainerConfig {
            engine: farhelm_proto::ContainerEngine::Podman,
            image_file: "/images/agent.oci".into(),
            api_proxy_port: 43210,
            github_token_file: Some("/tokens/github".into()),
        };
        draft.fields.container = Some(Some(config.clone()));
        assert_eq!(draft.refusal(None), None);
        assert!(template_summary(&draft.fields).contains("Containerized"));
        draft
            .fields
            .container
            .as_mut()
            .unwrap()
            .as_mut()
            .unwrap()
            .api_proxy_port = 0;
        assert!(draft.refusal(None).unwrap().contains("65535"));
        draft.fields.container = Some(Some(config));
        draft.fields.kind = Some(LauncherKind::Command);
        assert!(draft.refusal(None).unwrap().contains("Containerized"));
        Field::Container.remove(&mut draft.fields);
        assert_eq!(draft.fields.container, None);
        assert_eq!(draft.refusal(None), None);
    }

    /// Visible summaries preserve raw peer bytes until rendering, and only
    /// command arguments use mono. Names and model choices remain interface
    /// text even when they carry escaping and bidi requirements.
    #[test]
    fn summary_roles_keep_shell_values_separate_from_names() {
        let fields = TemplateFields {
            model: Some(Some("model\u{202e}".into())),
            name: Some("review\u{200b}".into()),
            permission_flags: Some(vec!["--mode".into(), "ask me".into()]),
            wrapper_prefix: Some(Some(vec!["wrapper".into(), "--".into()])),
            container: None,
            ..Default::default()
        };
        let parts = template_summary_parts(&fields);
        let technical: Vec<_> = parts
            .iter()
            .filter_map(|part| match part {
                DetailPart::MonoPeer(value) => Some(value.raw()),
                _ => None,
            })
            .collect();
        assert_eq!(technical, vec!["--mode 'ask me'", "wrapper --"]);
        let names: Vec<_> = parts
            .iter()
            .filter_map(|part| match part {
                DetailPart::Peer(value) => Some(value.raw()),
                _ => None,
            })
            .collect();
        assert_eq!(names, vec!["model\u{202e}", "review\u{200b}"]);
        assert_eq!(
            template_summary(&fields),
            "model<U+202E> · --mode 'ask me' · Wrapper prefix wrapper -- · Name review<U+200B>"
        );
    }

    /// Presence is the template edit, including an empty message that clears
    /// the receiving draft. Compatibility belongs to the selected agent;
    /// line breaks stay editable, while invisible formatting stays escaped.
    #[test]
    fn initial_prompt_field_preserves_presence_and_agent_compatibility() {
        let mut fields = Draft::default().fields;
        assert!(Field::Prompt.offered(&fields));
        Field::Prompt.add(&mut fields);
        assert_eq!(fields.prompt.as_deref(), Some(""));
        assert!(Field::Prompt.is_set(&fields));
        fields.prompt = Some("first\n\tsecond".into());
        assert_eq!(Field::Prompt.escaped_read_only_value(&fields), None);
        assert!(template_summary(&fields).contains("Initial prompt"));
        for harness in LaunchHarness::ALL {
            fields.agent = Some(*harness);
            assert_eq!(
                Field::Prompt.offered(&fields),
                harness.supports_initial_prompt(false)
            );
            assert_eq!(
                Field::Prompt.refusal(&fields, None).is_none(),
                harness.supports_initial_prompt(false)
            );
        }
        fields.kind = Some(LauncherKind::Command);
        assert!(!Field::Prompt.offered(&fields));
        fields.prompt = Some("first\u{202e}\nsecond".into());
        assert_eq!(
            Field::Prompt.escaped_read_only_value(&fields).as_deref(),
            Some("first<U+202E><U+000A>second")
        );
        Field::Prompt.remove(&mut fields);
        assert_eq!(fields.prompt, None);
    }

    /// All single-line peer-text fields share the read-only boundary, so a
    /// model, folder or session name cannot conceal bytes that commands protect.
    /// Ordinary Unicode, absent values and resets must stay editable.
    #[test]
    fn unsafe_editor_values_require_read_only_escaped_display() {
        for (raw, shown) in [
            ("model\u{200b}", "model<U+200B>"),
            ("/tmp/\u{202e}project", "/tmp/<U+202E>project"),
            ("first\r\nsecond", "first<U+000D><U+000A>second"),
        ] {
            let fields = TemplateFields {
                name: Some(raw.into()),
                model: Some(Some(raw.into())),
                destination: Some(TemplateDestination::Folder(raw.into())),
                command: Some(raw.into()),
                resume_command: Some(Some(raw.into())),
                ..Default::default()
            };
            for field in [
                Field::Name,
                Field::Model,
                Field::Destination,
                Field::Command,
                Field::Resume,
            ] {
                assert_eq!(
                    field.escaped_read_only_value(&fields).as_deref(),
                    Some(shown)
                );
            }
            let repository = TemplateFields {
                destination: Some(TemplateDestination::Github(raw.into())),
                ..Default::default()
            };
            assert_eq!(
                Field::Destination
                    .escaped_read_only_value(&repository)
                    .as_deref(),
                Some(shown)
            );
        }
        for raw in ["", "ordinary model", "/tmp/项目", "literal<U+200B>"] {
            let fields = TemplateFields {
                name: Some(raw.into()),
                model: Some(Some(raw.into())),
                destination: Some(TemplateDestination::Folder(raw.into())),
                command: Some(raw.into()),
                resume_command: Some(Some(raw.into())),
                ..Default::default()
            };
            for field in [
                Field::Name,
                Field::Model,
                Field::Destination,
                Field::Command,
                Field::Resume,
            ] {
                assert_eq!(field.escaped_read_only_value(&fields), None);
            }
        }
        let reset = TemplateFields {
            model: Some(None),
            resume_command: Some(None),
            ..Default::default()
        };
        for field in [
            Field::Name,
            Field::Model,
            Field::Destination,
            Field::Command,
            Field::Resume,
        ] {
            assert_eq!(
                field.escaped_read_only_value(&TemplateFields::default()),
                None
            );
            assert_eq!(field.escaped_read_only_value(&reset), None);
        }
    }

    /// Moving and renaming keys must preserve the helm's one-key-per-template
    /// contract, including a stale binding already naming the rename target.
    #[test]
    fn number_bindings_move_clear_and_follow_rename() {
        let mut bindings = BTreeMap::from([("1".into(), "a".into()), ("0".into(), "b".into())]);
        bind_number(&mut bindings, "a", Some("0"));
        assert_eq!(bindings, BTreeMap::from([("0".into(), "a".into())]));
        bind_number(&mut bindings, "b", Some("3"));
        rename_number_binding(&mut bindings, "a", "b");
        assert_eq!(bindings, BTreeMap::from([("0".into(), "b".into())]));
        rename_number_binding(&mut bindings, "absent", "b");
        assert_eq!(bindings, BTreeMap::from([("0".into(), "b".into())]));
        bind_number(&mut bindings, "b", None);
        assert!(bindings.is_empty());
    }

    /// A partial setup cannot become the shortcut's sole launch instruction.
    /// A command in an otherwise kindless template cannot satisfy the default
    /// agent tab; marking and number-key hints must agree with direct launch.
    #[test]
    fn quick_launch_requires_choices_for_the_active_kind() {
        assert!(!quick_launch_eligible(&TemplateFields::default()));
        assert!(!quick_launch_eligible(&TemplateFields {
            kind: Some(LauncherKind::Agent),
            ..Default::default()
        }));
        assert!(!quick_launch_eligible(&TemplateFields {
            command: Some(" \t".into()),
            ..Default::default()
        }));
        assert!(quick_launch_eligible(&TemplateFields {
            agent: Some(LaunchHarness::Claude),
            ..Default::default()
        }));
        assert!(!quick_launch_eligible(&TemplateFields {
            command: Some("agent --flag".into()),
            ..Default::default()
        }));
    }

    /// Composition judges the final active tab, while preserving choices in
    /// the other tab. A modifier alone must not launch an empty draft, and a
    /// later blank command must not inherit an earlier runnable command.
    #[test]
    fn stacked_quick_launch_tracks_order_and_dormant_choices() {
        let stacked_quick_launch_eligible = |templates: &[LaunchTemplate]| {
            farhelm_proto::launcher::stacked_quick_launch_eligible(
                templates.iter().map(|template| &template.fields),
            )
        };
        let template = |fields| LaunchTemplate {
            name: "setup".into(),
            fields,
        };
        let agent = template(TemplateFields {
            agent: Some(LaunchHarness::Codex),
            ..Default::default()
        });
        let modifier = template(TemplateFields {
            model: Some(Some("custom".into())),
            ..Default::default()
        });
        let command_tab = template(TemplateFields {
            kind: Some(LauncherKind::Command),
            agent: Some(LaunchHarness::Claude),
            ..Default::default()
        });
        let command = template(TemplateFields {
            command: Some("run-task".into()),
            ..Default::default()
        });
        let blank_command = template(TemplateFields {
            command: Some(" \t".into()),
            ..Default::default()
        });
        let agent_tab = template(TemplateFields {
            kind: Some(LauncherKind::Agent),
            ..Default::default()
        });
        assert!(!stacked_quick_launch_eligible(&[]));
        assert!(!stacked_quick_launch_eligible(std::slice::from_ref(
            &modifier
        )));
        assert!(stacked_quick_launch_eligible(&[agent.clone(), modifier]));
        assert!(!stacked_quick_launch_eligible(std::slice::from_ref(
            &command_tab
        )));
        assert!(!stacked_quick_launch_eligible(&[
            agent.clone(),
            command_tab.clone()
        ]));
        assert!(stacked_quick_launch_eligible(&[
            command_tab.clone(),
            command.clone()
        ]));
        assert!(!stacked_quick_launch_eligible(&[
            command_tab.clone(),
            command,
            blank_command
        ]));
        assert!(!stacked_quick_launch_eligible(&[
            command_tab.clone(),
            agent_tab.clone()
        ]));
        assert!(stacked_quick_launch_eligible(&[
            agent,
            command_tab,
            agent_tab
        ]));
    }

    /// Explicit supported choices must remain saveable even when their mode
    /// equals a request default. Otherwise saving a guarded launcher selection
    /// or an older OMP/Goose YOLO template fails only in the editor.
    #[test]
    fn explicit_supported_permissions_remain_saveable() {
        for &harness in LaunchHarness::ALL {
            if harness.offers_only_yolo() {
                continue;
            }
            for permission in LaunchPermission::ALL {
                if !harness.offers_permission(permission) {
                    continue;
                }
                let template = LaunchTemplate {
                    name: "explicit-permission".into(),
                    fields: TemplateFields {
                        kind: Some(LauncherKind::Agent),
                        agent: Some(harness),
                        permissions: Some(Some(permission)),
                        permission_flags: (permission == LaunchPermission::Custom)
                            .then(|| vec!["--mode".into(), "ask me".into()]),
                        ..Default::default()
                    },
                };
                assert_eq!(
                    saved_fields_refusal(&template, None),
                    None,
                    "{harness:?} {permission:?}"
                );
            }
        }
    }

    /// Both template selection surfaces share this summary. A host's model
    /// text must not hide or reverse the approval wording beside it; ordinary
    /// model ids and explicit defaults keep their established spelling.
    #[test]
    fn template_summaries_escape_model_text_before_permission_wording() {
        let fields = TemplateFields {
            model: Some(Some("model\u{200B}\u{202E}".into())),
            permissions: Some(Some(LaunchPermission::Yolo)),
            ..Default::default()
        };
        assert_eq!(
            template_summary(&fields),
            "model<U+200B><U+202E> · Approvals YOLO"
        );
        let ordinary = TemplateFields {
            model: Some(Some("ordinary-model".into())),
            ..Default::default()
        };
        assert_eq!(template_summary(&ordinary), "ordinary-model");
        let reset = TemplateFields {
            model: Some(None),
            ..Default::default()
        };
        assert_eq!(template_summary(&reset), "Model default");
    }

    /// Opening a kindless record changes only the draft. It must be visibly
    /// unsaved so simply inspecting a template cannot migrate its behavior.
    #[test]
    fn legacy_kind_is_inferred_without_changing_the_stored_template() {
        for (fields, kind) in [
            (
                TemplateFields {
                    agent: Some(LaunchHarness::Claude),
                    ..Default::default()
                },
                Some(LauncherKind::Agent),
            ),
            (
                TemplateFields {
                    yolo: Some(false),
                    ..Default::default()
                },
                Some(LauncherKind::Command),
            ),
            (
                TemplateFields {
                    resume_command: Some(None),
                    ..Default::default()
                },
                Some(LauncherKind::Command),
            ),
            (
                TemplateFields {
                    name: Some("review".into()),
                    ..Default::default()
                },
                None,
            ),
        ] {
            let stored = LaunchTemplate {
                name: "legacy".into(),
                fields: fields.clone(),
            };
            let draft = Draft::from_template(&stored);
            assert_eq!(stored.fields, fields, "opening is not a write");
            assert_eq!(draft.fields, TemplateFields { kind, ..fields });
            assert_eq!(
                draft
                    != Draft {
                        name: stored.name,
                        fields: stored.fields,
                        wrapper_input: None,
                        container_input: None,
                    },
                kind.is_some()
            );
        }
    }

    /// Opening a template leaves stored data readable but removes retired trust
    /// from its editable draft, so both ordinary saves and duplicates drop it.
    #[test]
    fn legacy_trust_is_dropped_from_editor_drafts() {
        for trust in [None, Some(false), Some(true)] {
            let stored = LaunchTemplate {
                name: "legacy trust".into(),
                fields: TemplateFields {
                    kind: Some(LauncherKind::Agent),
                    agent: Some(LaunchHarness::Codex),
                    workspace_trust: Some(trust),
                    ..Default::default()
                },
            };
            let draft = Draft::from_template(&stored);
            assert_eq!(stored.fields.workspace_trust, Some(trust));
            assert_eq!(draft.fields.workspace_trust, None);
            assert_eq!(draft.fields.agent, stored.fields.agent);
            assert!(!template_summary(&stored.fields).contains("trust"));
        }
    }

    /// A don't-switch template cannot hide launch-specific edits inside a
    /// placement template, and a switch change preserves the old value until
    /// its author removes it rather than silently making a different template.
    #[test]
    fn switches_offer_their_fields_and_flag_retained_incompatible_values() {
        let placement = TemplateFields::default();
        let offered: Vec<_> = Field::ALL
            .into_iter()
            .filter(|f| f.offered(&placement))
            .collect();
        assert_eq!(offered, vec![Field::Host, Field::Destination, Field::Name]);
        let mut draft = Draft {
            name: "command".into(),
            fields: TemplateFields {
                kind: Some(LauncherKind::Command),
                command: Some("tool {farhelm_args}".into()),
                yolo: Some(false),
                ..Default::default()
            },
            wrapper_input: None,
            container_input: None,
        };
        assert!(Field::Resume.offered(&draft.fields));
        assert!(!Field::Effort.offered(&draft.fields));
        assert_eq!(
            draft.refusal(None),
            None,
            "a template need not name an agent"
        );
        draft.fields.kind = Some(LauncherKind::Agent);
        assert!(Field::Model.offered(&draft.fields));
        assert!(draft.refusal(None).unwrap().contains("Command"));
        assert_eq!(draft.fields.command.as_deref(), Some("tool {farhelm_args}"));
        Field::Command.remove(&mut draft.fields);
        Field::Yolo.remove(&mut draft.fields);
        assert_eq!(draft.refusal(None), None);
    }

    /// Agent-specific approval vocabularies must not leak into another agent's
    /// template. Model text is intentionally exempt: the editor accepts custom
    /// ids while the launcher retains its own application rules.
    #[test]
    fn agent_changes_flag_approvals_but_keep_arbitrary_models() {
        let mut draft = Draft {
            name: "agent".into(),
            fields: TemplateFields {
                kind: Some(LauncherKind::Agent),
                agent: Some(LaunchHarness::Goose),
                permissions: Some(Some(LaunchPermission::SmartApprove)),
                model: Some(Some("any future model".into())),
                ..Default::default()
            },
            wrapper_input: None,
            container_input: None,
        };
        assert_eq!(draft.refusal(None), None);
        draft.fields.agent = Some(LaunchHarness::Claude);
        assert!(draft.refusal(None).unwrap().contains("Approvals"));
        assert_eq!(
            draft.fields.permissions,
            Some(Some(LaunchPermission::SmartApprove))
        );
        draft.fields.permissions = Some(None);
        assert_eq!(draft.refusal(None), None);
        assert_eq!(
            permission_options(Some(LaunchHarness::Claude)),
            vec![
                (
                    "default".into(),
                    "Guarded · Claude auto mode (reset)".into()
                ),
                ("guarded".into(), "Guarded · Claude auto mode".into()),
                ("yolo".into(), "YOLO".into()),
                ("custom".into(), "Custom".into())
            ]
        );
        assert!(
            permission_options(Some(LaunchHarness::Omp))
                .iter()
                .any(|(key, _)| key == "approve")
        );
        assert!(
            !permission_options(Some(LaunchHarness::Omp))
                .iter()
                .any(|(key, _)| key == "smart_approve")
        );
        draft.fields.agent = Some(LaunchHarness::Pi);
        assert!(!Field::Permissions.offered(&draft.fields));
        assert!(draft.refusal(None).is_some());
        Field::Permissions.remove(&mut draft.fields);
        assert_eq!(draft.refusal(None), None);
    }

    /// Approvals-only templates cannot assume which agent will receive them.
    /// Every declared choice is editable and saveable without an agent; the
    /// launcher's existing compatibility check decides whether it applies.
    #[test]
    fn partial_templates_offer_every_approval_choice() {
        for permission in LaunchPermission::ALL {
            assert!(
                permission_options(None)
                    .iter()
                    .any(|(key, _)| key == &word(permission))
            );
            let draft = Draft {
                name: "approvals".into(),
                fields: TemplateFields {
                    kind: Some(LauncherKind::Agent),
                    permissions: Some(Some(permission)),
                    permission_flags: (permission == LaunchPermission::Custom)
                        .then(|| vec!["--mode".into()]),
                    ..Default::default()
                },
                wrapper_input: None,
                container_input: None,
            };
            assert_eq!(draft.refusal(None), None);
            check_template_shape(&LaunchTemplate {
                name: draft.name,
                fields: draft.fields,
            })
            .unwrap();
        }
    }

    /// Presence must survive editing for null resets and false approvals alike.
    /// Removing a field makes it absent; adding it again does not recover an
    /// older incompatible value or require any other launcher field.
    #[test]
    fn field_presence_preserves_resets_false_and_values() {
        let fields = TemplateFields {
            kind: Some(LauncherKind::Command),
            yolo: Some(false),
            resume_command: Some(None),
            destination: Some(TemplateDestination::Github("acme/web".into())),
            ..Default::default()
        };
        let template = LaunchTemplate {
            name: "roundtrip".into(),
            fields: fields.clone(),
        };
        let mut draft = Draft::from_template(&template);
        assert_eq!(draft.fields, fields);
        assert!(Field::Yolo.is_set(&draft.fields));
        assert!(Field::Resume.is_set(&draft.fields));
        Field::Resume.remove(&mut draft.fields);
        assert!(!Field::Resume.is_set(&draft.fields));
        Field::Resume.add(&mut draft.fields);
        assert_eq!(draft.fields.resume_command, Some(None));
        assert_eq!(draft.refusal(None), None);
        assert_eq!(draft.fields.kind, Some(LauncherKind::Command));
    }

    /// Copy names fit the API's byte limit even for multibyte names. They do
    /// not replace an existing copy, and remain mere suggestions until save.
    #[test]
    fn duplicate_names_are_free_and_fit_the_name_limit() {
        assert_eq!(
            duplicate_name("build", &["build copy".into(), "build copy 2".into()]),
            "build copy 3"
        );
        let copy = duplicate_name(&"界".repeat(42), &[]);
        assert!(copy.len() <= 128);
        assert!(copy.ends_with(" copy"));
    }

    /// Spec: the delete kind a render computes for the label ("delete" offers
    /// Undo, "delete permanently" does not) decides the Undo offer on its own:
    /// `undo_after_delete` takes that kind and the clicked row and never
    /// re-checks the template's shape. The mounted-control regression below
    /// separately covers native listener replacement before the next paint;
    /// a browser applies display edits during its render and misses that gap.
    ///
    /// Why: the label comes from one render's list, and the handler used to
    /// re-run the shape check on the list as it stood at the click. A reload
    /// landing in between (the user's own repair being saved, say) could make
    /// the click contradict the label the user acted on.
    #[test]
    fn delete_offers_undo_for_the_kind_the_label_showed() {
        let stored = |session_name: &str| LaunchTemplate {
            name: "legacy".into(),
            fields: TemplateFields {
                name: Some(session_name.into()),
                ..Default::default()
            },
        };
        // A zero-width space fails today's session-name rule; a plain name
        // passes. Both are stored under the same template name.
        let unsaveable = stored("hidden\u{200B}name");
        let repaired = stored("plain");
        let undo = |shown, template: &LaunchTemplate| {
            undo_after_delete(shown, template.clone(), 7).map(|u| (u.template, u.generation))
        };

        let shown = DeleteKind::of(Some(std::slice::from_ref(&unsaveable)), Some("legacy"));
        assert_eq!(shown, DeleteKind::Permanent);
        assert_eq!(
            undo(shown, &repaired),
            None,
            "a click after the template was repaired still deletes permanently"
        );

        let shown = DeleteKind::of(Some(std::slice::from_ref(&repaired)), Some("legacy"));
        assert_eq!(shown, DeleteKind::Restorable);
        assert_eq!(
            undo(shown, &unsaveable),
            Some((unsaveable.clone(), 7)),
            "a click labelled \"delete\" offers Undo of the value it deleted"
        );

        // No list, or no row for the open template, labels an ordinary
        // delete; the handler itself refuses when it finds no stored row.
        assert_eq!(DeleteKind::of(None, Some("legacy")), DeleteKind::Restorable);
        assert_eq!(
            DeleteKind::of(Some(std::slice::from_ref(&unsaveable)), Some("other")),
            DeleteKind::Restorable
        );
    }

    /// The desktop dispatch path can update listeners before the webview paints.
    /// Keep its serialized-event fixture native, like the renderer it models.
    #[cfg(native_desktop)]
    mod native_delete {
        use super::*;
        use dioxus::core::ElementId;

        /// A real resource's externally completed reads expose Retry without a
        /// network peer, a process environment change or a scheduler delay.
        struct DeleteFixture {
            reads:
                RefCell<std::collections::VecDeque<futures_channel::oneshot::Receiver<DeleteRows>>>,
            resource: Cell<Option<Resource<DeleteRows>>>,
            frame: Cell<DeleteFrame>,
            renders: Cell<usize>,
            calls: RefCell<Vec<DeleteKind>>,
        }

        type DeleteRows = Result<Vec<LaunchTemplate>, String>;

        /// The list result actually consumed by the fixture's last native render.
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        enum DeleteFrame {
            Missing,
            Failed,
            Plain,
            Permanent,
        }

        /// Use the production classification and controls with an owned list source.
        /// Like the dialog, pending is peeked; Retry need not paint a loading frame.
        fn delete_fixture_app(fixture: Rc<DeleteFixture>) -> Element {
            let source = fixture.clone();
            let mut templates = use_resource(move || {
                let next = source
                    .reads
                    .borrow_mut()
                    .pop_front()
                    .expect("an owned list read");
                async move {
                    next.await
                        .expect("the test completes every started list read")
                }
            });
            fixture.resource.set(Some(templates));
            fixture.renders.set(fixture.renders.get() + 1);
            let list = templates.read().clone();
            let kind = DeleteKind::of(
                list.as_ref()
                    .and_then(|rows| rows.as_ref().ok())
                    .map(Vec::as_slice),
                Some("legacy"),
            );
            fixture.frame.set(match &list {
                None => DeleteFrame::Missing,
                Some(Err(_)) => DeleteFrame::Failed,
                Some(Ok(_)) if kind == DeleteKind::Permanent => DeleteFrame::Permanent,
                Some(Ok(_)) => DeleteFrame::Plain,
            });
            let recorded = fixture.clone();
            rsx! {
                TemplateDeleteButtons {
                    kind, disabled: templates.pending(),
                    on_delete: move |kind| recorded.calls.borrow_mut().push(kind),
                }
                if matches!(list, Some(Err(_))) {
                    button { class: "templates-retry", onclick: move |_| templates.restart(), "Retry" }
                }
            }
        }

        /// A completed oneshot needs only local scheduler/render work. Keep it
        /// bounded, and retain the deliberate native-update-without-paint boundary.
        fn render_delete_frame(dom: &mut VirtualDom, fixture: &DeleteFixture, wanted: DeleteFrame) {
            for _ in 0..16 {
                dom.process_events();
                let _unpainted = dom.render_immediate_to_vec();
                if fixture.frame.get() == wanted {
                    return;
                }
            }
            panic!(
                "wanted {wanted:?}, last native render was {:?}",
                fixture.frame.get()
            );
        }

        /// Inspect production button markup rather than assuming a sibling count.
        /// This also lets the old single-control negative control reach dispatch.
        fn visible_delete_fixture_target(dom: &VirtualDom, class: &str, label: &str) -> ElementId {
            use dioxus::core::{AttributeValue, DynamicNode, TemplateAttribute, TemplateNode};

            /// Read the label for either fixed controls or the changing-label
            /// negative control, without assuming how rsx stores its text.
            fn label_text(node: &TemplateNode, vnode: &VNode) -> String {
                match node {
                    TemplateNode::Text { text } => (*text).into(),
                    TemplateNode::Element { children, .. } => children
                        .iter()
                        .map(|child| label_text(child, vnode))
                        .collect(),
                    TemplateNode::Dynamic { id } => match &vnode.dynamic_nodes[*id] {
                        DynamicNode::Text(text) => text.value.clone(),
                        DynamicNode::Fragment(children) => children
                            .iter()
                            .flat_map(|child| {
                                child
                                    .template
                                    .roots
                                    .iter()
                                    .map(|node| label_text(node, child))
                            })
                            .collect(),
                        _ => String::new(),
                    },
                }
            }

            /// Static classes live in the template, not the mutation stream.
            fn collect(
                dom: &VirtualDom,
                vnode: &VNode,
                class: &str,
                found: &mut Vec<(ElementId, String, bool)>,
            ) {
                for (index, attrs) in vnode.dynamic_attrs.iter().enumerate() {
                    if !attrs.iter().any(|attr| attr.name == "onclick") {
                        continue;
                    }
                    let path = vnode.template.attr_paths[index];
                    let mut node = &vnode.template.roots[usize::from(path[0])];
                    for &step in &path[1..] {
                        let TemplateNode::Element { children, .. } = node else {
                            panic!("button path crosses a non-element")
                        };
                        node = &children[usize::from(step)];
                    }
                    let TemplateNode::Element {
                        tag,
                        attrs: fixed,
                        children,
                        ..
                    } = node
                    else {
                        continue;
                    };
                    let matching = fixed.iter().any(|attr| matches!(attr,
                    TemplateAttribute::Static { name: "class", value, .. } if value.split_whitespace().any(|word| word == class)
                ));
                    // Dioxus can split one element's attributes into several
                    // groups. Its listener group need not contain visibility.
                    let flag = |name| {
                        vnode
                            .dynamic_attrs
                            .iter()
                            .enumerate()
                            .filter(|(index, _)| vnode.template.attr_paths[*index] == path)
                            .flat_map(|(_, attrs)| attrs.iter())
                            .any(|attr| {
                                attr.name == name
                                    && matches!(attr.value, AttributeValue::Bool(true))
                            })
                    };
                    if matching && !flag("hidden") {
                        assert_eq!(*tag, "button");
                        let text = children
                            .iter()
                            .map(|child| label_text(child, vnode))
                            .collect::<String>();
                        found.push((
                            vnode
                                .mounted_dynamic_attribute(index, dom)
                                .expect("mounted button"),
                            text,
                            flag("disabled"),
                        ));
                    }
                }
                for (index, node) in vnode.dynamic_nodes.iter().enumerate() {
                    match node {
                        DynamicNode::Fragment(children) => {
                            for child in children {
                                collect(dom, child, class, found);
                            }
                        }
                        DynamicNode::Component(component) => {
                            if let Some(scope) = component.mounted_scope(index, vnode, dom) {
                                collect(dom, scope.root_node(), class, found);
                            }
                        }
                        _ => {}
                    }
                }
            }
            let mut found = Vec::new();
            collect(
                dom,
                dom.get_scope(ScopeId::APP)
                    .expect("fixture mounted")
                    .root_node(),
                class,
                &mut found,
            );
            assert_eq!(found.len(), 1, "one visible {class} control");
            let (id, text, disabled) = found.remove(0);
            assert_eq!(text, label, "premise: this is the painted label");
            assert!(!disabled, "premise: the painted control accepts clicks");
            id
        }

        /// Deliver the webview's captured element identity to the current native
        /// listener table, without applying any intervening display mutations.
        fn click_delete_fixture(dom: &VirtualDom, id: ElementId) {
            let data: Rc<dyn std::any::Any> = Rc::new(dioxus::html::PlatformEventData::new(
                Box::new(dioxus::html::SerializedMouseData::default()),
            ));
            dom.runtime()
                .handle_event("click", dioxus::core::Event::new(data, true), id);
        }

        /// A failed follow-up read and successful Retry must not install permanent
        /// deletion under the plain button still painted by the desktop webview.
        /// The reverse transition must likewise keep the no-Undo promise. A single
        /// changing-kind control fails at the observed callback kind in this test.
        #[farhelm_testtrace::test]
        fn retry_keeps_the_kind_of_the_painted_delete_element() {
            dioxus::html::set_event_converter(Box::new(dioxus::html::SerializedHtmlEventConverter));
            let mut replies = Vec::new();
            let mut reads = std::collections::VecDeque::new();
            for _ in 0..4 {
                let (send, receive) = futures_channel::oneshot::channel();
                replies.push(send);
                reads.push_back(receive);
            }
            let mut replies = replies.into_iter();
            let fixture = Rc::new(DeleteFixture {
                reads: RefCell::new(reads),
                resource: Cell::new(None),
                frame: Cell::new(DeleteFrame::Missing),
                renders: Cell::new(0),
                calls: RefCell::new(Vec::new()),
            });
            let specimen = |name: &str| LaunchTemplate {
                name: "legacy".into(),
                fields: TemplateFields {
                    name: Some(name.into()),
                    ..Default::default()
                },
            };
            let valid = specimen("plain");
            let invalid = specimen("hidden\u{200B}name");
            assert!(check_template_shape(&valid).is_ok());
            assert!(check_template_shape(&invalid).is_err());
            let mut dom = VirtualDom::new_with_props(delete_fixture_app, fixture.clone());
            dom.rebuild_to_vec();
            replies
                .next()
                .unwrap()
                .send(Ok(vec![valid.clone()]))
                .unwrap();
            render_delete_frame(&mut dom, &fixture, DeleteFrame::Plain);
            visible_delete_fixture_target(&dom, "templates-delete", "Delete");

            let mut resource = fixture.resource.get().expect("resource mounted");
            dom.in_runtime(|| resource.restart());
            replies
                .next()
                .unwrap()
                .send(Err("follow-up list read failed".into()))
                .unwrap();
            render_delete_frame(&mut dom, &fixture, DeleteFrame::Failed);
            let old_plain = visible_delete_fixture_target(&dom, "templates-delete", "Delete");
            let retry = visible_delete_fixture_target(&dom, "templates-retry", "Retry");
            let renders = fixture.renders.get();
            click_delete_fixture(&dom, retry);
            dom.process_events();
            assert!(resource.pending(), "Retry started a real read");
            assert_eq!(
                fixture.renders.get(),
                renders,
                "no intermediate loading frame was painted"
            );
            replies
                .next()
                .unwrap()
                .send(Ok(vec![invalid.clone()]))
                .unwrap();
            render_delete_frame(&mut dom, &fixture, DeleteFrame::Permanent);
            click_delete_fixture(&dom, old_plain);
            assert_eq!(*fixture.calls.borrow(), [DeleteKind::Restorable]);
            assert!(undo_after_delete(fixture.calls.borrow()[0], invalid, 1).is_some());

            // The new state is now the conceptual painted frame. Its permanent
            // element must not inherit the ordinary handler from a later render.
            let old_permanent =
                visible_delete_fixture_target(&dom, "templates-delete", "Delete permanently");
            dom.in_runtime(|| resource.restart());
            replies
                .next()
                .unwrap()
                .send(Ok(vec![valid.clone()]))
                .unwrap();
            render_delete_frame(&mut dom, &fixture, DeleteFrame::Plain);
            click_delete_fixture(&dom, old_permanent);
            assert_eq!(
                *fixture.calls.borrow(),
                [DeleteKind::Restorable, DeleteKind::Permanent]
            );
            assert!(undo_after_delete(fixture.calls.borrow()[1], valid, 2).is_none());
        }
    }

    /// Mount a leave-prompt slot the way `TemplatesDialog` does and run `body`
    /// against it inside the runtime, so the answer tests below exercise the
    /// real signal-backed slot. The dialog itself is not mounted: its
    /// handlers only route the clicks to the `answer_*` helpers tested here.
    fn with_leave_prompt(body: impl FnOnce(ConfirmSlot<Departure>)) {
        std::thread_local! {
            static SLOT: Cell<Option<ConfirmSlot<Departure>>> = const { Cell::new(None) };
        }

        fn app() -> Element {
            let slot = use_confirm_slot::<Departure, ()>();
            SLOT.with(|cell| cell.set(Some(slot)));
            rsx! {}
        }

        let mut dom = VirtualDom::new(app);
        dom.rebuild_in_place();
        let slot = SLOT.with(Cell::get).expect("the slot mounted");
        dom.in_runtime(|| body(slot));
    }

    /// A stored template to head to, distinct per `name`.
    fn open_departure(name: &str) -> Departure {
        Departure::Open(Box::new(LaunchTemplate {
            name: name.into(),
            fields: TemplateFields::default(),
        }))
    }

    /// Spec (`answer_save_and_leave`, `answer_discard`,
    /// `answer_keep_editing`): an answer to "Save these changes before
    /// leaving?" drawn for a showing that is no longer open does nothing:
    /// "save" saves nothing and leaves nowhere, "discard" discards nothing,
    /// "keep editing" closes nothing, and the prompt now on screen (if any)
    /// stays open. A showing stops being open once answered, once another
    /// departure replaces it while it is up, and once it is reopened, even
    /// for the same departure.
    ///
    /// Why: these clicks can be handled after the click or key that changed
    /// the prompt but before the render that redraws it. A Discard handled
    /// after "keep editing" used to throw away the draft the user had just
    /// chosen to keep, and one handled after Escape replaced the prompt
    /// would leave for the earlier destination instead of the one the user
    /// last chose (Escape's Close).
    #[farhelm_testtrace::test]
    fn a_stale_leave_prompt_answer_does_nothing() {
        with_leave_prompt(|mut slot| {
            // "keep editing" first; a Discard or save from the same showing
            // queued behind it must not act.
            let first = slot.open(open_departure("build"), ());
            assert!(answer_keep_editing(&mut slot, &first, false));
            assert_eq!(answer_discard(&mut slot, &first, false), None);
            assert_eq!(answer_save_and_leave(&slot, &first), None);
            assert!(!answer_keep_editing(&mut slot, &first, false));
            assert!(!slot.is_open(), "no prompt is open after keep editing");

            // Escape replaces Back's prompt with Close's while it is up.
            let back = slot.open(Departure::Back, ());
            let close = slot.open(Departure::Close, ());
            assert_eq!(answer_discard(&mut slot, &back, false), None);
            assert_eq!(answer_save_and_leave(&slot, &back), None);
            assert!(!answer_keep_editing(&mut slot, &back, false));
            assert!(slot.is_current(&close), "the replacing prompt stays up");

            // Reopened for an equal departure: still a different showing.
            let reopened = slot.open(Departure::Close, ());
            assert_eq!(answer_discard(&mut slot, &close, false), None);
            assert_eq!(answer_save_and_leave(&slot, &close), None);
            assert!(!answer_keep_editing(&mut slot, &close, false));
            assert!(slot.is_current(&reopened), "the reopened prompt stays up");
        });
    }

    /// Spec: a current answer acts on the departure its showing was opened
    /// with, which is what the prompt's render drew. "save" leaves the prompt
    /// open (it stays up through the save, and after a failed save the user
    /// answers the same showing again); while the dialog is busy with a
    /// write (that save included), "discard" and "keep editing" are refused
    /// and the prompt stays; otherwise "discard"
    /// yields the departure and "keep editing" closes the prompt, both
    /// consuming the showing.
    ///
    /// Why: the save path owns the prompt until its outcome is known, so a
    /// Discard or "keep editing" queued behind "save" in one burst must not
    /// leave or close underneath it (the buttons are already drawn disabled
    /// by then). And each answer must go where the prompt it was drawn for
    /// said, not wherever a later departure points.
    #[farhelm_testtrace::test]
    fn leave_prompt_answers_act_on_the_departure_drawn() {
        with_leave_prompt(|mut slot| {
            let heading = open_departure("review");
            let shown = slot.open(heading.clone(), ());
            assert_eq!(answer_save_and_leave(&slot, &shown), Some(heading.clone()));
            assert!(
                slot.is_current(&shown),
                "the prompt stays up through the save"
            );

            // While that save runs.
            assert_eq!(answer_discard(&mut slot, &shown, true), None);
            assert!(!answer_keep_editing(&mut slot, &shown, true));
            assert!(slot.is_current(&shown), "the save still owns the prompt");

            // After a failed save: retry, then discard instead.
            assert_eq!(answer_save_and_leave(&slot, &shown), Some(heading.clone()));
            assert_eq!(answer_discard(&mut slot, &shown, false), Some(heading));
            assert!(!slot.is_open(), "discard consumed the showing");

            let staying = slot.open(Departure::New, ());
            assert!(answer_keep_editing(&mut slot, &staying, false));
            assert!(!slot.is_open(), "keep editing closed the prompt");
        });
    }

    /// Spec: a new template, or a rename, is saved only once the template
    /// list has loaded with no read in progress and shows the name is free;
    /// while the list is loading (the first load or a reload) or failed to
    /// load, the save is refused saying so. Saving the template already open
    /// under its own name is never refused.
    ///
    /// Why: the panel used to read an unloaded list as "no template has this
    /// name", so saving a new template while the list was slow or failing
    /// replaced an existing template the user never opened (SPEC_impl.md
    /// "Templates" requires refusing a name another template has). A reload
    /// counts as loading because the list it still shows can lack the
    /// template just saved; the save handler maps a reload to `Loading`.
    #[test]
    fn a_new_or_renamed_template_needs_the_loaded_list() {
        let loaded = ListedNames::Loaded(vec!["build", "review"]);
        let taken =
            |refusal: Option<String>| refusal.is_some_and(|text| text.contains("already exists"));
        let loading =
            |refusal: Option<String>| refusal.is_some_and(|text| text.contains("still loading"));
        let failed = |refusal: Option<String>| {
            refusal.is_some_and(|text| text.contains("could not be loaded"))
        };

        // A new template: a taken name is refused as taken, a free one is
        // allowed, and either is refused while the list is not loaded.
        assert!(taken(save_name_refusal(None, "build", &loaded)));
        assert_eq!(save_name_refusal(None, "deploy", &loaded), None);
        for name in ["build", "deploy"] {
            assert!(
                loading(save_name_refusal(None, name, &ListedNames::Loading)),
                "{name}"
            );
            assert!(
                failed(save_name_refusal(None, name, &ListedNames::Failed)),
                "{name}"
            );
        }

        // A rename is a new name too.
        assert!(taken(save_name_refusal(Some("build"), "review", &loaded)));
        assert_eq!(save_name_refusal(Some("build"), "deploy", &loaded), None);
        assert!(loading(save_name_refusal(
            Some("build"),
            "deploy",
            &ListedNames::Loading
        )));
        assert!(failed(save_name_refusal(
            Some("build"),
            "deploy",
            &ListedNames::Failed
        )));

        // The template open under its own name saves whatever the list.
        for listed in [loaded.clone(), ListedNames::Loading, ListedNames::Failed] {
            assert_eq!(
                save_name_refusal(Some("build"), "build", &listed),
                None,
                "{listed:?}"
            );
        }
    }
}
