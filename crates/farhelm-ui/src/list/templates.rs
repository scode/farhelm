//! Edit launch templates without inventing values for fields they leave alone.
//!
//! The dialog keeps one explicit draft and a saved baseline. Stored kindless
//! templates are interpreted only in the draft: that difference is unsaved
//! until the user chooses save. API writes keep the existing last-write-wins
//! contract; name checks prevent an ordinary new/rename/undo from replacing a
//! template the user did not open.

use super::shared::HostOption;
use crate::ApiBase;
use crate::api::{PreferenceValue, Preferences};
use crate::launch_sequence::TEMPLATE_SHORTCUTS;
use crate::menu_panel::{
    MenuFocusQueue, MenuOpenIntent, MenuOrder, MenuWiring, PanelPlacement, ROW_MENU_OUTSIDE_RELAY,
    cancel_menu_focus, closed_toggle_key_intent, focus_menu_toggle, forget_menu_focus,
    handle_menu_key, install_row_menu_outside_dismiss, measurement_outcome, record_menu_item_focus,
    remember_menu_item, session_menu_placement_style,
};
use crate::modal_dialog::{DialogFocus, DialogHandle, ModalDialog};
use crate::peer::{PeerText, display_peer};
use dioxus::prelude::*;
pub(super) use farhelm_proto::launcher::quick_launch_eligible;
use farhelm_proto::launcher::{
    LaunchCatalogModel, LaunchTemplate, LauncherKind, TEMPLATE_NAME_CAP, TemplateDestination,
    TemplateFields, TemplateNameKind, check_template_shape,
};
use farhelm_proto::{LaunchHarness, LaunchPermission};
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
pub(super) fn apply_template_bindings(current: &mut Preferences, fresh: Preferences) {
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

/// A row-local popup stays inside modal isolation while using the app's menu
/// keyboard, placement and outside-dismiss machinery. Escape closes this menu
/// without bubbling into the containing Templates dialog.
#[component]
fn NumberShortcutMenu(
    name: String,
    launchable: bool,
    disabled: bool,
    bindings: BTreeMap<String, PeerText>,
    layout_epoch: ReadSignal<u64>,
    dialog_epoch: ReadSignal<u64>,
    on_choose: EventHandler<Option<String>>,
) -> Element {
    let bound = bindings
        .iter()
        .find_map(|(key, owner)| (owner.raw() == name).then(|| key.clone()));
    let mut open = use_signal(|| false);
    let mut placement = use_signal(|| PanelPlacement::Unmeasured);
    let mut toggle = use_signal(|| None::<Rc<MountedData>>);
    let mut handles = use_signal(HashMap::new);
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
    // Fixed coordinates become stale when either the dialog or its ancestors
    // move. Opening is deliberately not a dependency; the popup's own scroller
    // also leaves these epochs untouched, so its ten choices stay reachable.
    use_effect(move || {
        layout_epoch();
        dialog_epoch();
        if *open.peek() {
            close_menu.call(());
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
            button { r#type: "button", class: "templates-number-key",
                disabled,
                aria_label: "number shortcut {display_peer(&name)}", aria_haspopup: "menu", aria_expanded: open(),
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
                        div { class: "session-row-menu-items", role: "menu", aria_label: "number key for {display_peer(&name)}",
                            for key in order.actions() {
                                button { r#type: "button", class: "btn session-row-menu-item templates-number-option", role: "menuitem",
                                    tabindex: if focused().and_then(|position| order.get(position)).or(order.get(0)) == Some(key) { "0" } else { "-1" },
                                    "data-menu-focus": crate::menu_panel::menu_item_focus_key(wiring, key),
                                    "data-tooltip": if let Some(key) = key { format!("give this template {}", TEMPLATE_SHORTCUTS.number_chord(key)) } else { "remove this template's number shortcut".into() },
                                    onmounted: move |element| remember_menu_item(wiring, key, element.data()),
                                    onfocusin: move |_| record_menu_item_focus(wiring, key), onfocusout: move |_| focused.set(None),
                                    onkeydown: move |event| { event.stop_propagation(); handle_menu_key(&event, order.position(key), wiring, &()); },
                                    onclick: move |_| { close_menu.call(()); on_choose.call(key.map(str::to_string)); },
                                    span { if let Some(key) = key { "{TEMPLATE_SHORTCUTS.number_chord(key)}" } else { "no shortcut" } }
                                    if let Some(owner) = key.and_then(|key| bindings.get(key)) {
                                        span { class: "templates-number-owner peer-value", dir: "ltr",
                                            if owner.raw() == name { "this template" } else { "{owner.escaped()} (moves here)" }
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
/// Reuse the dialog installer's focus memory, and leave a usable focused control
/// alone. Two frames let explicit navigation/prompt focus win before fallback.
fn restore_template_focus() {
    document::eval(&format!(
        r#"requestAnimationFrame(() => requestAnimationFrame(() => {{
            const dialog = document.querySelector('{DIALOG_SELECTOR}');
            if (!dialog) return;
            const usable = el => el instanceof HTMLElement && el.isConnected
                && dialog.contains(el) && !el.disabled && el.getClientRects().length;
            if (usable(document.activeElement)) return;
            const last = dialog.__farhelmLastFocus;
            const name = dialog.querySelector('.templates-name');
            (usable(last) ? last : usable(name) ? name
                : dialog.querySelector('.templates-new') ?? dialog).focus({{preventScroll: true}});
        }}))"#,
    ));
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
    pub(super) const ALL: [Self; 12] = [
        Self::Command,
        Self::Yolo,
        Self::Agent,
        Self::Model,
        Self::Effort,
        Self::Permissions,
        Self::Wrapper,
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
            Self::Prompt => "initial prompt",
            Self::Agent => "agent type",
            Self::Model => "model",
            Self::Effort => "effort",
            Self::Permissions | Self::Yolo => "approvals",
            Self::Wrapper => "wrapper prefix",
            Self::Command => "command",
            Self::Resume => "resume command",
            Self::Host => "host",
            Self::Destination => "destination",
            Self::Name => "session name",
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
            Self::Wrapper => f.kind == Some(LauncherKind::Agent),
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
    let mut options = vec![(DEFAULT.into(), "default".into())];
    options.extend(efforts.into_iter().map(|v| (word(v), word(v))));
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
        }
    }

    /// Refuse incompatible or unfinished fields without requiring a complete
    /// launch. Another stacked template may supply the missing agent or command.
    fn refusal(&self, catalog: Option<&[LaunchCatalogModel]>) -> Option<String> {
        if let Some(text) = &self.wrapper_input {
            return Some(
                farhelm_proto::split_wrapper_prefix(text)
                    .err()
                    .unwrap_or_else(|| {
                        "wrapper prefix changed while saving; review it and try again".to_string()
                    }),
            );
        }
        if let Err(reason) = farhelm_proto::check_permission_flags(
            self.fields.permissions.flatten(),
            self.fields.permission_flags.as_deref(),
        ) {
            return Some(reason);
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
                let verb = if label == "host" { "choose" } else { "type" };
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
            "the template list is still loading, so Farhelm cannot check that no other template has this name; \
             wait for it to load, then save again"
                .to_string(),
        ),
        ListedNames::Failed => Some(
            "the template list could not be loaded, so Farhelm cannot check that no other template has this name; \
             retry loading it above, then save again"
                .to_string(),
        ),
        ListedNames::Loaded(names) => names.contains(&name).then(|| {
            format!("a template named {name:?} already exists; edit that one, or choose another name")
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

/// A one-line description of what a template sets, for its row in the
/// list: every field it sets, a reset shown as such. The command and resume
/// text are left out (they can be long, and the edit form shows them).
/// Model ids can originate on a host, so escape them before composing the
/// approval wording shared by the list and quick switcher.
pub(super) fn template_summary(fields: &TemplateFields) -> String {
    let choice = |name: &str, value: Option<String>| match value {
        Some(value) => format!("{name} {value}"),
        None => format!("{name} default"),
    };
    let mut parts = Vec::new();
    if fields.prompt.is_some() {
        parts.push("initial prompt".to_string());
    }
    if let Some(kind) = fields.kind {
        parts.push(format!("{} launch", word(kind)));
    }
    if let Some(agent) = fields.agent {
        parts.push(crate::launch_composer::harness_label(agent).to_string());
    }
    if let Some(model) = &fields.model {
        parts.push(match model {
            Some(model) => crate::peer::display_peer(model),
            None => "model default".to_string(),
        });
    }
    if let Some(effort) = fields.effort {
        parts.push(choice("effort", effort.map(word)));
    }
    if let Some(permissions) = fields.permissions {
        parts.push(format!(
            "approvals {}",
            crate::launch_composer::permission_label(fields.agent, permissions)
        ));
    }
    if let Some(words) = &fields.permission_flags {
        parts.push(crate::peer::display_peer(&shell_words::join(words)));
    }
    if let Some(prefix) = &fields.wrapper_prefix {
        parts.push(match prefix {
            Some(words) => format!(
                "wrapper prefix {}",
                crate::peer::display_peer(&shell_words::join(words))
            ),
            None => "wrapper prefix none".to_string(),
        });
    }
    if fields.command.is_some() {
        parts.push("command".to_string());
    }
    if let Some(yolo) = fields.yolo {
        parts.push(if yolo { "YOLO" } else { "not YOLO" }.to_string());
    }
    if let Some(resume) = &fields.resume_command {
        parts.push(
            if resume.is_some() {
                "resume command"
            } else {
                "resume default"
            }
            .to_string(),
        );
    }
    if fields.host.is_some() {
        parts.push("host".to_string());
    }
    if fields.destination.is_some() {
        parts.push("destination".to_string());
    }
    if let Some(name) = &fields.name {
        parts.push(format!("name {}", display_peer(name)));
    }
    if parts.is_empty() {
        "sets nothing yet".to_string()
    } else {
        parts.join(" · ")
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
        div { class: "templates-segments template-name-kind", role: "group", aria_label: "session name kind",
            for (value, text) in [(None, "base name"), (Some(TemplateNameKind::Fixed), "fixed name")] {
                button { r#type: "button", disabled, aria_pressed: fixed == value.is_some(),
                    "data-tooltip": "session name: {text}. {NAME_KIND_HELP}",
                    onclick: move |_| on_change.call(value), "{text}"
                }
            }
        }
    }
}

/// One native select, including an older incompatible value until the user
/// corrects it. Dropping an unknown option would visually claim a different
/// choice while the draft still held the old one. Its explicit accessible name
/// stays independent of the option text nested inside the visible label.
/// Options also mark the saved selection: a select's initial value can be applied
/// before its options mount, which otherwise leaves the first option displayed.
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
        label { class: "templates-field",
            span { "{label}" }
            select { disabled, value: "{value}", aria_label: "{label}", "data-tooltip": "{label}: the value this template applies",
                onchange: move |event| on_change.call(event.value()),
                for (key, text) in options { option { value: "{key}", selected: key == value, "{text}" } }
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
    let refusal = field.refusal(&current, catalog.as_deref());
    let label = field.label();
    let controls = match field {
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
                    span { "wrapper prefix" }
                    input { class: "templates-wrapper templates-code", r#type: "text", value,
                        disabled, readonly: text_read_only, autocomplete: "off", autocorrect: "off", autocapitalize: "none", spellcheck: "false",
                        placeholder: "with-endpoint --profile work --",
                        "data-tooltip": "wrapper prefix: words placed before Farhelm's agent command; no placeholders",
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
                span { "initial prompt" }
                textarea { class: "templates-prompt", rows: 4,
                    value: escaped.unwrap_or_else(|| current.prompt.clone().unwrap_or_default()),
                    disabled, readonly: text_read_only,
                    autocomplete: "off", autocorrect: "off", autocapitalize: "none", spellcheck: "false",
                    "data-tooltip": "initial prompt: the first message on each launch from this template",
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
                    span { "model" }
                    input { class: "templates-model", r#type: "text", value: "{value}", disabled, readonly: text_read_only, placeholder: "default, or type any model",
                        autocomplete: "off", autocorrect: "off", autocapitalize: "none", spellcheck: "false",
                        "data-tooltip": "model: type any model; suggestions follow the agent type this template sets",
                        oninput: move |event| { if !text_read_only { draft.with_mut(|d| d.fields.model = Some((!event.value().is_empty()).then(|| event.value()))); } },
                    }
                }
                if !suggestions.is_empty() {
                    div { class: "templates-model-suggestions", role: "group", aria_label: "model suggestions",
                        for id in suggestions {
                            button { r#type: "button", class: "templates-suggestion", disabled,
                                "data-tooltip": "set model to {display_peer(&id)}", onclick: move |_| draft.with_mut(|d| d.fields.model = Some(Some(id.clone()))), "{display_peer(&id)}"
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
                options: vec![("false".into(), "keeps safety checks".into()), ("true".into(), "YOLO".into())], disabled,
                on_change: move |v: String| draft.with_mut(|d| d.fields.yolo = v.parse().ok()),
            }
            span { class: "templates-hint", "Your statement about this command; Farhelm does not inspect it." }
        },
        Field::Command => rsx! {
            label { class: "templates-field",
                span { "command" }
                input { class: "templates-command templates-code", value: escaped.unwrap_or_else(|| current.command.unwrap_or_default()), disabled, readonly: text_read_only, placeholder: "agent-tool {{farhelm_args}}",
                    autocomplete: "off", autocorrect: "off", autocapitalize: "none", spellcheck: "false",
                    "data-tooltip": "command: the command this template puts in the launcher", oninput: move |e| { if !text_read_only { draft.with_mut(|d| d.fields.command = Some(e.value())); } },
                }
            }
            span { class: "templates-hint", "Placeholders: {{cwd}} for the folder and {{farhelm_args}} for Farhelm's integration arguments; each must be a whole argument." }
        },
        Field::Resume => rsx! {
            label { class: "templates-field",
                span { "resume command" }
                input { class: "templates-resume templates-code", value: escaped.unwrap_or_else(|| current.resume_command.flatten().unwrap_or_default()), disabled, readonly: text_read_only, placeholder: "default, or agent-tool --resume {{conversation}} {{farhelm_args}}",
                    autocomplete: "off", autocorrect: "off", autocapitalize: "none", spellcheck: "false",
                    "data-tooltip": "resume command: an empty input resets it to the default", oninput: move |e| { if !text_read_only { draft.with_mut(|d| d.fields.resume_command = Some((!e.value().is_empty()).then(|| e.value()))); } },
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
                options.insert(0, (String::new(), "choose a host".into()));
            } else if !options.iter().any(|(id, _)| id == &value) {
                options.push((value.clone(), "an install no host here has".into()));
            }
            rsx! { Choice { label, value, options, disabled, on_change: move |v: String| draft.with_mut(|d| d.fields.host = Some(v)) } }
        }
        Field::Name => rsx! {
            div { class: "templates-field",
                div { class: "template-name-label",
                    label { r#for: "template-session-name", "session name" }
                    NameKindControl { kind: current.name_kind, disabled,
                        on_change: move |kind| draft.with_mut(|d| d.fields.name_kind = kind)
                    }
                }
                input { id: "template-session-name", aria_label: "session name", class: "templates-session-name", value: escaped.unwrap_or_else(|| current.name.unwrap_or_default()), disabled, readonly: text_read_only, placeholder: "review",
                    autocomplete: "off", autocorrect: "off", autocapitalize: "none", spellcheck: "false",
                    "data-tooltip": "session name: the name this template puts in the launcher", oninput: move |e| { if !text_read_only { draft.with_mut(|d| d.fields.name = Some(e.value())); } },
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
                    span { "destination" }
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
                        span { if github { "repository" } else { "folder path" } }
                        input { class: "templates-destination", value: escaped.unwrap_or(value), disabled, readonly: text_read_only, placeholder: if github { "owner/name" } else { "/path/to/project" },
                            autocomplete: "off", autocorrect: "off", autocapitalize: "none", spellcheck: "false",
                            "data-tooltip": if github { "repository: each session gets a managed checkout" } else { "folder path: the working folder on the selected host" },
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
            button { class: "templates-remove", r#type: "button", disabled, aria_label: "remove {label}", "data-tooltip": "remove {label}: this template will leave it as is",
                onclick: move |_| draft.with_mut(|d| { field.remove(&mut d.fields); if field == Field::Wrapper { d.wrapper_input = None; } }), "✕"
            }
        }
    }
}

/// All editor departure paths share the same guard, including mobile Back and
/// Escape. A pending action carries its destination rather than losing it when
/// the user saves from the inline prompt.
#[derive(Clone, PartialEq)]
enum Departure {
    // Only Open carries a stored template. Keep the pending navigation value
    // small as template fields grow; the other departures carry no payload.
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
    mut pending: Signal<Option<Departure>>,
    mut error: Signal<Option<String>>,
    names: Vec<String>,
    on_close: EventHandler<()>,
) {
    let focus = match &action {
        Departure::Back => Some(".templates-new"),
        Departure::Close => None,
        _ => Some(".templates-name"),
    };
    pending.set(None);
    error.set(None);
    match action {
        Departure::Open(template) => {
            baseline.set(Draft {
                name: template.name.clone(),
                fields: template.fields.clone(),
                wrapper_input: None,
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

/// A deleted template lives only in this mounted dialog. The generation keeps
/// an older expiry timer from clearing a later deletion's undo notice.
#[derive(Clone, PartialEq)]
struct Undo {
    template: LaunchTemplate,
    generation: u64,
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
    let mut pending = use_signal(|| None::<Departure>);
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
                    if let Some(value) = value {
                        crate::api::store_preference(&base, value);
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
    // Delete removes the stored snapshot, even when the unsaved draft has
    // repaired an old value. Undo would have to save that original snapshot.
    let permanent_delete = list
        .as_ref()
        .and_then(|r| r.as_ref().ok())
        .and_then(|rows| rows.iter().find(|t| previous().as_ref() == Some(&t.name)))
        .is_some_and(|t| check_template_shape(t).is_err());
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
            pending.set(Some(action));
        } else {
            leave_editor(
                action,
                draft,
                baseline,
                previous,
                opened,
                pending,
                error,
                departure_names.clone(),
                on_close,
            );
        }
    });

    let save_base = base.clone();
    let save: EventHandler<Option<Departure>> = EventHandler::new(move |after| {
        if busy() {
            return;
        }
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
        .err();
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
        let saving_request = crate::api::put_template(&base, &value.name, &value.fields);
        spawn(async move {
            let result = saving_request.await;
            let result = match (result, old) {
                (Ok(()), Some(old)) if old != value.name => {
                    // Another window may have marked this name, or moved the
                    // mark away, since sign-in. Refresh only this field while
                    // respecting our unconfirmed explicit queue choice. Rename
                    // remains ordinary last-writer-wins writes across clients.
                    let fetched = match crate::api::fetch_preferences(&base).await {
                        Ok(fetched) => fetched,
                        Err(message) => {
                            busy.set(false);
                            error.set(Some(format!("the new name was saved, but shortcut bindings could not be read; the old template was kept: {message}")));
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
                            busy.set(false);
                            error.set(Some("the new name was saved, but the quick launch mark could not be moved; the old template was kept".into()));
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
                            busy.set(false);
                            error.set(Some("the new name was saved, but its number key could not be moved; the old template was kept".into()));
                            templates.restart();
                            return;
                        }
                    }
                    let deleting_request = crate::api::delete_template(&base, &old);
                    if !crate::api::prepare_template_delete(&old).await {
                        busy.set(false);
                        error.set(Some("the new name was saved, but a shortcut write was interrupted; the old template was kept".into()));
                        templates.restart();
                        return;
                    }
                    deleting_request.await
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
                            action, draft, baseline, previous, opened, pending, error, names,
                            on_close,
                        );
                    } else {
                        pending.set(None);
                    }
                }
                Err(message) => error.set(Some(message)),
            }
        });
    });

    let delete_base = base.clone();
    let delete = move |_| {
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
                "reload the template list before deleting this template".into(),
            ));
            return;
        };
        // Restoration uses the ordinary save path, which refuses old values
        // outside today's shape rules. Do not offer an Undo that cannot work.
        let restorable = check_template_shape(&template).is_ok();
        let base = delete_base.clone();
        busy.set(true);
        error.set(None);
        let deleting_request = crate::api::delete_template(&base, &name);
        spawn(async move {
            if !crate::api::prepare_template_delete(&name).await {
                busy.set(false);
                error.set(Some(
                    "a shortcut write was interrupted; try deleting the template again".into(),
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
                    undo.set(restorable.then_some(Undo {
                        template,
                        generation,
                    }));
                    leave_editor(
                        Departure::Back,
                        draft,
                        baseline,
                        previous,
                        opened,
                        pending,
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
                Err(message) => error.set(Some(message)),
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
                    "cannot check the deleted template's name: {message}"
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
    let prompt = pending();
    let inferred =
        previous().is_some() && baseline().fields.kind.is_none() && current.fields.kind.is_some();
    rsx! {
        div { class: "host-settings-backdrop", role: "presentation",
            // The capped dialog can move while keeping its own size. Its
            // full-window backdrop also observes width-only viewport changes.
            onresize: move |_| dialog_epoch += 1,
            ModalDialog { class: if opened() { "templates-dialog templates-editor-open" } else { "templates-dialog" }, role: "dialog", aria_modal: "true", aria_label: "templates", tabindex: "-1",
                handle: DialogHandle::new(DIALOG_SELECTOR),
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
                div { class: "templates-header",
                    h2 { class: "host-settings-title", "templates" }
                    button { r#type: "button", class: "btn btn-neutral templates-close", disabled, "data-tooltip": "close templates; unsaved changes ask before leaving", onclick: move |_| depart.call(Departure::Close), "close" }
                }
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
                    aside { class: "templates-sidebar", aria_label: "template list", aria_busy: templates.pending(),
                        button { r#type: "button", class: "btn btn-neutral templates-new", disabled, "data-tooltip": "start a new template; unsaved changes ask before leaving", onclick: move |_| depart.call(Departure::New), "+ new template" }
                        match list.clone() {
                            None => rsx! { p { class: "templates-hint", "loading templates" } },
                            Some(Err(_)) => rsx! {},
                            Some(Ok(rows)) => rsx! {
                                ul { class: "templates-list",
                                    for template in rows {
                                        li { key: "{template.name}",
                                            button {
                                                r#type: "button", class: "templates-quick-mark",
                                                // A lit bolt stays clickable even when its template has since
                                                // become ineligible (edited through the API, say): clicking
                                                // the lit bolt is how a mark is cleared, and a disabled one
                                                // would leave an unusable mark with no way to turn it off.
                                                disabled: disabled || templates.pending() || (!quick_launch_eligible(&template.fields) && preferences.0.read().quick_launch_template.as_ref().map(PeerText::raw) != Some(template.name.as_str())),
                                                aria_label: "quick launch {display_peer(&template.name)}",
                                                aria_pressed: preferences.0.read().quick_launch_template.as_ref().map(PeerText::raw) == Some(template.name.as_str()),
                                                "data-tooltip": if preferences.0.read().quick_launch_template.as_ref().map(PeerText::raw) == Some(template.name.as_str()) { format!("{quick_launch_chord} starts a session from this template right away, without opening New. Only one template can hold ⚡. Click to turn off.") } else if !quick_launch_eligible(&template.fields) { format!("{quick_launch_chord} starts a session from the ⚡ template right away, without opening New. Only one template can hold ⚡. Can't mark this one: an agent launch needs an agent; a command launch needs a nonblank command.") } else { format!("{quick_launch_chord} starts a session from the ⚡ template right away, without opening New. Only one template can hold ⚡. Click to mark this one.") },
                                                onclick: { let name = template.name.clone(); move |_| { let lit = preferences.0.read().quick_launch_template.as_ref().map(PeerText::raw) == Some(name.as_str()); edit_binding.call(BindingEdit::Bolt(name.clone(), !lit)) } },
                                                "⚡"
                                            }
                                            NumberShortcutMenu {
                                                key: "number-{template.name}", name: template.name.clone(),
                                                launchable: quick_launch_eligible(&template.fields), disabled: disabled || templates.pending(),
                                                bindings: preferences.0.read().template_shortcuts.clone(),
                                                layout_epoch, dialog_epoch,
                                                on_choose: { let name = template.name.clone(); move |key| edit_binding.call(BindingEdit::Number(name.clone(), key)) },
                                            }
                                            button { r#type: "button", class: "templates-row", disabled: disabled || templates.pending(), aria_pressed: previous().as_ref()==Some(&template.name), "data-tooltip": "edit template {display_peer(&template.name)}", onclick: move |_| depart.call(Departure::Open(Box::new(template.clone()))),
                                                span { class: "templates-row-name peer-value", dir: "ltr", "{display_peer(&template.name)}" }
                                                span { class: "templates-row-summary", "{template_summary(&template.fields)}" }
                                                if preferences.0.read().quick_launch_template.as_ref().map(PeerText::raw) == Some(template.name.as_str()) { span { class: "templates-quick-chord", "{quick_launch_chord}" } }
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
                    section { class: "templates-editor", aria_label: "template editor",
                        if opened() {
                            button { r#type: "button", class: "templates-back", disabled, "data-tooltip": "back to templates; unsaved changes ask before leaving", onclick: move |_| depart.call(Departure::Back), "‹ templates" }
                            div { class: "templates-editor-heading",
                                h3 { if previous().is_some() { "edit template" } else { "new template" } }
                                if dirty { span { class: "templates-unsaved", "unsaved changes" } }
                            }
                            label { class: "templates-field",
                                span { "name" }
                                // The name is a literal tl: search key; browser
                                // correction would change what the user chose.
                                input { class: "templates-name", value: shown_name, readonly: name_read_only, disabled, placeholder: "my-claude",
                                    autocomplete: "off", autocorrect: "off", autocapitalize: "none", spellcheck: "false",
                                    "data-tooltip": "template name: use tl:name to apply it in New session", oninput: move |e| { if !name_read_only { draft.with_mut(|d| d.name=e.value()); } } }
                            }
                            if name_read_only {
                                p { class: "templates-hint", "This name contains invisible formatting, shown as escapes. It stays unchanged until replaced." }
                                button { r#type: "button", class: "btn btn-neutral templates-replace-name", disabled,
                                    onclick: move |_| { draft.with_mut(|d| d.name.clear()); focus_template_control(".templates-name"); }, "replace name" }
                            }
                            p { class: "templates-quick-state",
                                if previous().is_some() && preferences.0.read().quick_launch_template.as_ref().map(PeerText::raw) == previous().as_deref() {
                                    "⚡ quick launch: {quick_launch_chord} starts a session from this saved template."
                                } else if quick_launch_eligible(&current.fields) {
                                    "Not the quick launch. The ⚡ in the list makes it one."
                                } else {
                                    "Can't be the quick launch: an agent launch needs an agent; a command launch needs a nonblank command."
                                }
                            }
                            div { class: "templates-field",
                                span { "switches launcher to" }
                                div { class: "templates-segments", role: "group", aria_label: "switches launcher to",
                                    for (kind,text) in [(Some(LauncherKind::Agent),"agent"),(Some(LauncherKind::Command),"command"),(None,"don't switch")] {
                                        button { r#type: "button", disabled, aria_pressed: current.fields.kind==kind, "data-tooltip": "switches launcher to: {text}; incompatible fields stay visible", onclick: move |_| { draft.with_mut(|d| d.fields.kind=kind); add_open.set(false); }, "{text}" }
                                    }
                                }
                            }
                            if inferred { p { class: "templates-hint templates-inferred", "This stored template does not switch the launcher yet. Saving will store the switch inferred from its fields." } }
                            h4 { class: "templates-sets-heading", "sets" }
                            for field in Field::ALL.into_iter().filter(|f| f.is_set(&current.fields)) {
                                FieldEditor { key: "{editor_source}-{field.key()}", field, draft, hosts: hosts.clone(), catalog: catalog_rows.clone(), disabled }
                            }
                            div { class: "templates-add-area",
                                button { r#type: "button", class: "btn btn-neutral templates-add", disabled, aria_expanded: add_open(), "data-tooltip": "add a field this template will set; fields not shown are left as is", onclick: move |_| add_open.toggle(), "+ add field" }
                                if add_open() {
                                    div { class: "templates-add-menu", role: "group", aria_label: "add field",
                                        for field in Field::ALL.into_iter().filter(|f| !f.is_set(&current.fields) && f.offered(&current.fields)) {
                                            button { r#type: "button", disabled, "data-tooltip": "add {field.label()} to the template", onclick: move |_| { draft.with_mut(|d| field.add(&mut d.fields)); add_open.set(false); }, "{field.label()}" }
                                        }
                                    }
                                }
                            }
                            p { class: "templates-hint", "Fields not shown are left as is. Another template can supply choices this one leaves out." }
                            footer { class: "templates-editor-footer",
                                if let Some(action)=prompt.clone() {
                                    div { class: "templates-departure", role: "status",
                                        onmounted: move |_| { document::eval("requestAnimationFrame(() => { const prompt = document.querySelector('.templates-departure'); prompt?.scrollIntoView({block: 'nearest'}); prompt?.querySelector('.templates-save-leave')?.focus({preventScroll: true}); })"); },
                                        p { "Save these changes before leaving?" }
                                        div { class: "templates-actions",
                                            button { r#type: "button", class: "btn btn-primary templates-save-leave", disabled, "data-tooltip": "save these changes, then continue", onclick: { let action=action.clone(); move |_| save.call(Some(action.clone())) }, "save" }
                                            button { r#type: "button", class: "btn btn-neutral templates-discard", disabled, "data-tooltip": "discard these changes, then continue", onclick: { let names=names.clone(); move |_| leave_editor(action.clone(),draft,baseline,previous,opened,pending,error,names.clone(),on_close) }, "discard" }
                                            button { r#type: "button", class: "btn btn-neutral templates-keep-editing", disabled, "data-tooltip": "stay here with the unsaved draft", onclick: move |_| { pending.set(None); focus_template_control(".templates-name"); }, "keep editing" }
                                        }
                                    }
                                } else {
                                    div { class: "templates-actions",
                                        if previous().is_some() {
                                            button { r#type: "button", class: "btn btn-neutral templates-duplicate", disabled: disabled || templates.pending(), "data-tooltip": "open a new unsaved copy with a free name", onclick: move |_| depart.call(Departure::Duplicate), "duplicate" }
                                            button { r#type: "button", class: "btn btn-danger templates-delete", disabled: disabled || templates.pending(), "data-tooltip": if permanent_delete { "delete this template now; it fails current save rules, so undo is not available" } else { "delete this template now; undo is available here for about ten seconds" }, onclick: delete, if permanent_delete { "delete permanently" } else { "delete" } }
                                        }
                                        button { r#type: "button", class: "btn btn-primary templates-save", disabled, "data-tooltip": "save the template's name, switch and shown fields", onclick: move |_| save.call(None), "save" }
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
                        p { class: "templates-error", "templates unavailable: {message}" }
                        button { r#type: "button", class: "btn btn-neutral templates-retry", disabled, "data-tooltip": "load the template list again without discarding this draft", onclick: move |_| templates.restart(), "retry" }
                    }
                }
                if let Some(message)=error() { p { class: "templates-error", role: "status", "{message}" } }
                if let Some(value)=undo() {
                    div { class: "templates-undo-notice", role: "status",
                        span { "Deleted {display_peer(&value.template.name)}." }
                        button { r#type: "button", class: "btn btn-neutral templates-undo", disabled, "data-tooltip": "restore the deleted template if its old name is still free", onclick: restore, "undo" }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert!(template_summary(&fields).contains("wrapper prefix none"));
        fields.wrapper_prefix = Some(Some(vec!["wrapper".into(), "--".into()]));
        assert!(template_summary(&fields).contains("wrapper prefix wrapper --"));
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
        assert!(template_summary(&fields).contains("initial prompt"));
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
            "model<U+200B><U+202E> · approvals yolo"
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
        assert_eq!(template_summary(&reset), "model default");
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
        assert!(draft.refusal(None).unwrap().contains("command"));
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
        };
        assert_eq!(draft.refusal(None), None);
        draft.fields.agent = Some(LaunchHarness::Claude);
        assert!(draft.refusal(None).unwrap().contains("approvals"));
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
                ("yolo".into(), "yolo".into()),
                ("custom".into(), "custom".into())
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
