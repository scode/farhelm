//! The Tensorlake recipe editor owns one editing session inside its host modal.
//!
//! Settings → Tensorlake swaps its recipe list for it and keeps its answers
//! sticky inside that panel. Add host opens it in place for "New recipe…",
//! with deletion disabled, and receives a scrolling body and pinned footer
//! as direct siblings. Neither layout creates a nested modal or takes over
//! its host's focus boundary. The editor owns the draft and its baseline,
//! local validation, the save or delete request, and
//! the "Save these changes before leaving?" question when Back or Cancel would
//! drop unsaved edits. The host only decides what happens after it reports
//! saved, cancelled or deleted.
//!
//! Requests follow the explicit-save pattern of the ntfy form: they are
//! declared in the click handler and spawned with `spawn_forever`, so closing
//! Settings does not retract an accepted save, and they touch component state
//! only while it is still mounted. Endpoint tokens and Codex logins select
//! names from the metadata catalog; values never enter the recipe draft or save.
//!
//! Wording follows the goal's UI decisions: the built artifact is a
//! "snapshot", never a "template" (that word means launch templates), and the
//! systemd requirement for reused images is spelled out in user terms.

use std::{cell::Cell, future::Future, pin::Pin, rc::Rc};

use dioxus::prelude::*;
use farhelm_proto::tensorlake::recipes::{
    CreateRecipe, EndpointSave, EndpointView, RecipeSave, RecipeSize, RecipeSource, RecipeView,
    Tunnel, check_recipe, check_recipe_name, parse_packages,
};

use crate::ApiBase;
use crate::api::{create_recipe, delete_recipe, update_recipe};
use crate::modal_dialog::{DialogActions, DialogFooter};
use crate::ops::use_confirm_slot;

// ---------------------------------------------------------------------------
// The draft: what the form holds, as text
// ---------------------------------------------------------------------------

/// Where the draft's software comes from. Kept apart from the per-kind text
/// fields so switching kinds back and forth never loses what was typed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Software {
    Packages,
    Image,
    Snapshot,
}

/// One endpoint's URL and named credential selection, with no secret draft.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct EndpointDraft {
    base_url: String,
    token_credential: String,
}

/// Codex keeps each mode's draft while only the selected mode reaches the
/// recipe. None and Subscription both stop selecting Farhelm's API provider.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CodexMode {
    None,
    Endpoint,
    Subscription,
}

/// One tunnel row as typed. Ports stay text until save so a half-typed row
/// does not fight the input.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct TunnelDraft {
    sandbox_port: String,
    host: String,
    port: String,
}

/// The editor's whole form state. Equality with the baseline (the form as
/// opened) is what "unsaved changes" means, so a different credential selection
/// counts as a change and an edit typed back to its original does not.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Draft {
    name: String,
    cpus: String,
    memory_gb: String,
    disk_gb: String,
    software: Software,
    packages: String,
    image: String,
    snapshot_id: String,
    claude: EndpointDraft,
    codex: EndpointDraft,
    codex_mode: CodexMode,
    codex_login_credential: String,
    github_token_credential: String,
    tunnels: Vec<TunnelDraft>,
}

impl Draft {
    /// A new recipe's starting values: a small size inside the Usage Credits
    /// plan's limits, and managed software with no extra packages.
    fn new_recipe() -> Self {
        Self {
            name: String::new(),
            cpus: "2".into(),
            memory_gb: "4".into(),
            disk_gb: "20".into(),
            software: Software::Packages,
            packages: String::new(),
            image: String::new(),
            snapshot_id: String::new(),
            claude: EndpointDraft::default(),
            codex: EndpointDraft::default(),
            codex_mode: CodexMode::None,
            codex_login_credential: String::new(),
            github_token_credential: String::new(),
            tunnels: Vec::new(),
        }
    }

    /// The form for an existing recipe. Packages show one per line;
    /// whatever separators were typed originally are not stored.
    fn from_view(view: &RecipeView) -> Self {
        let mut draft = Self::new_recipe();
        draft.name = view.name.clone();
        draft.cpus = view.size.cpus.to_string();
        draft.memory_gb = view.size.memory_gb.to_string();
        draft.disk_gb = view.size.disk_gb.to_string();
        match &view.source {
            RecipeSource::Packages { packages } => {
                draft.software = Software::Packages;
                draft.packages = packages.join("\n");
            }
            RecipeSource::Image { image } => {
                draft.software = Software::Image;
                draft.image = image.clone();
            }
            RecipeSource::Snapshot { snapshot_id } => {
                draft.software = Software::Snapshot;
                draft.snapshot_id = snapshot_id.clone();
            }
        }
        let endpoint = |view: &Option<EndpointView>| EndpointDraft {
            base_url: view
                .as_ref()
                .map(|endpoint| endpoint.base_url.clone())
                .unwrap_or_default(),
            token_credential: view
                .as_ref()
                .and_then(|endpoint| endpoint.token_credential.clone())
                .unwrap_or_default(),
        };
        draft.claude = endpoint(&view.claude);
        draft.codex = endpoint(&view.codex);
        draft.codex_mode = if view.codex.is_some() {
            CodexMode::Endpoint
        } else if view.codex_login_credential.is_some() {
            CodexMode::Subscription
        } else {
            CodexMode::None
        };
        draft.codex_login_credential = view.codex_login_credential.clone().unwrap_or_default();
        draft.github_token_credential = view.github_token_credential.clone().unwrap_or_default();
        draft.tunnels = view
            .tunnels
            .iter()
            .map(|tunnel| TunnelDraft {
                sandbox_port: tunnel.sandbox_port.to_string(),
                host: tunnel.host.clone(),
                port: tunnel.port.to_string(),
            })
            .collect();
        draft
    }

    /// The save request this draft describes, or the first problem a user
    /// can fix in the form, checked with the same rules the helm applies.
    /// Messages are lowercase like the helm's; the caller capitalizes them.
    fn to_save(&self) -> Result<RecipeSave, String> {
        let number = |text: &str, what: &str| {
            text.trim()
                .parse::<u32>()
                .map_err(|_| format!("{what} must be a whole number"))
        };
        let port = |text: &str| {
            text.trim()
                .parse::<u16>()
                .map_err(|_| "tunnel ports must be from 1 to 65535".to_string())
        };
        let size = RecipeSize {
            cpus: number(&self.cpus, "CPUs")?,
            memory_gb: number(&self.memory_gb, "memory")?,
            disk_gb: number(&self.disk_gb, "disk")?,
        };
        let source = match self.software {
            Software::Packages => RecipeSource::Packages {
                packages: parse_packages(&self.packages),
            },
            Software::Image => RecipeSource::Image {
                image: self.image.trim().to_string(),
            },
            Software::Snapshot => RecipeSource::Snapshot {
                snapshot_id: self.snapshot_id.trim().to_string(),
            },
        };
        let tunnels = self
            .tunnels
            .iter()
            .map(|row| {
                Ok(Tunnel {
                    sandbox_port: port(&row.sandbox_port)?,
                    host: row.host.trim().to_string(),
                    port: port(&row.port)?,
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        let codex = if self.codex_mode == CodexMode::Endpoint {
            Some(endpoint_save(&self.codex).ok_or("Codex API endpoint needs a base URL")?)
        } else {
            None
        };
        let codex_login_credential = if self.codex_mode == CodexMode::Subscription {
            if self.codex_login_credential.is_empty() {
                return Err("choose a Codex login in Subscription mode".into());
            }
            Some(self.codex_login_credential.clone())
        } else {
            None
        };
        let save = RecipeSave {
            size,
            source,
            claude: endpoint_save(&self.claude),
            codex,
            codex_login_credential,
            github_token_credential: (!self.github_token_credential.is_empty())
                .then(|| self.github_token_credential.clone()),
            tunnels,
        };
        check_recipe(&save)?;
        Ok(save)
    }
}

/// The save shape of one endpoint draft.
///
/// A blank base URL removes the endpoint's reference; the catalog value stays
/// saved. Unlike an unsaved secret, a name needs no discard warning of its own.
fn endpoint_save(draft: &EndpointDraft) -> Option<EndpointSave> {
    let base_url = draft.base_url.trim();
    if base_url.is_empty() {
        return None;
    }
    Some(EndpointSave {
        base_url: base_url.to_string(),
        token_credential: (!draft.token_credential.is_empty())
            .then(|| draft.token_credential.clone()),
    })
}

// ---------------------------------------------------------------------------
// The component
// ---------------------------------------------------------------------------

/// A dialog's close request, held back while an embedded editor has unsaved
/// changes.
///
/// The editor lives inside Settings or Add host, and that modal's Escape and
/// close button would otherwise unmount it and drop the draft without the
/// "Save these changes before leaving?"
/// question its own Back and Cancel ask. The enclosing dialog provides one
/// guard ([`Self::provide`]) and routes every close through
/// [`Self::request`]; a mounted editor registers itself. The close the user
/// asked for is handed to the editor, so Save or Discard can finish it after
/// answering; "Keep editing" drops it.
///
/// One editor at a time: the recipe list mounts at most one, and a guard
/// registered by an editor is removed only by that same editor's unmount.
#[derive(Clone, Copy, PartialEq)]
pub(crate) struct DialogCloseGuard {
    guard: Signal<Option<Callback<Callback<()>, bool>>>,
}

impl DialogCloseGuard {
    /// Provide an empty guard to the calling component's subtree. A hook:
    /// call it unconditionally, once, in the dialog component.
    pub(crate) fn provide() -> Self {
        use_context_provider(|| Self {
            guard: Signal::new(None),
        })
    }

    /// Ask to close: run `close` now unless an editor holds it back. Returns
    /// whether it was held, in which case the editor's question is now open
    /// and `close` runs only if the user then saves or discards.
    pub(crate) fn request(&self, close: Callback<()>) -> bool {
        let guard = *self.guard.peek();
        // A registered editor answers whether it held the close; one with
        // nothing to lose declines, and the close goes ahead here.
        let held = guard.is_some_and(|guard| guard.call(close));
        if !held {
            close.call(());
        }
        held
    }
}

/// A save or delete in flight: boxed because create and update return
/// different future types.
type Request<T> = Pin<Box<dyn Future<Output = Result<T, String>>>>;

/// Hand a newly mounted editor focus without repeating the host modal's setup.
///
/// Add host starts at Name. Settings reveals the editor's top, including its
/// Back button, and focuses the heading without native nearest-focus scrolling
/// moving it again. Run only on mount: input rerenders must not
/// jump the viewport or take focus from a field or confirmation answer.
/// The section supplies its own leading padding; adding the body's padding
/// would leave a strip of preceding account text visible above the editor.
/// The deferred frame captures this exact editor, so a later replacement does
/// not inherit a retired opening's focus handoff. It yields to a field or
/// control already chosen in this opening: stealing focus between selection
/// and text input can leave the draft unchanged even though typing began.
fn focus_editor(in_add_host: bool) {
    let dialog = if in_add_host {
        ".add-host-form"
    } else {
        ".app-settings-dialog[aria-label=\"Settings\"]"
    };
    let dialog = serde_json::to_string(dialog).expect("a selector always serializes");
    document::eval(&format!(
        r#"(() => {{
            const editor = document.querySelector({dialog})?.querySelector('.recipe-editor');
            requestAnimationFrame(() => {{
                if (!editor?.isConnected || !editor.getClientRects().length) return;
                const active = document.activeElement;
                // The mount handoff is only an initial destination. A field or
                // another modal control may already own the next input by now.
                if (active instanceof HTMLElement && active !== editor.closest('[role="dialog"]')
                    && (editor.contains(active) || active.matches('button, input, textarea, select, a[href], [contenteditable]'))
                    && !active.matches(':disabled') && !active.closest('[hidden], [inert]')
                    && active.getClientRects().length
                    && !['hidden', 'collapse'].includes(getComputedStyle(active).visibility)) return;
                const heading = editor.querySelector('.recipe-editor-title');
                const body = editor.closest('.host-settings-body');
                if (!{in_add_host} && heading && body) {{
                    body.scrollTop += editor.getBoundingClientRect().top - body.getBoundingClientRect().top;
                }}
                const target = {in_add_host} ? editor.querySelector('.recipe-name') : heading;
                target?.focus({{ preventScroll: true }});
            }});
        }})();"#,
    ));
}

/// One recipe's editor. `existing: None` edits a new recipe.
///
/// `in_add_host` supplies the host dialog's body and footer as direct siblings;
/// the default Settings layout keeps its answers inside the editor so hiding
/// the Tensorlake panel hides them too. Both share the same editing state.
///
/// Reports through its handlers and never closes itself: `on_saved` after a
/// successful save (with what the helm stored), `on_cancel` when the user
/// leaves (after the unsaved-changes question, when there were changes), and
/// `on_deleted` after a delete. Delete is offered only with `allow_delete`
/// and only for an existing recipe. Handlers are not called once the editor
/// has been unmounted (Settings closed while a request ran); the list
/// re-reads through the invalidation feed instead.
#[component]
pub(crate) fn RecipeEditor(
    existing: Option<RecipeView>,
    on_saved: EventHandler<RecipeView>,
    on_cancel: EventHandler<()>,
    #[props(default)] in_add_host: bool,
    #[props(default)] allow_delete: bool,
    #[props(default)] on_deleted: Option<EventHandler<String>>,
) -> Element {
    let base = use_context::<ApiBase>().0;
    let catalog = crate::credentials_settings::use_catalog();
    let opened = existing.clone();
    let baseline = use_hook(|| {
        opened
            .as_ref()
            .map_or_else(Draft::new_recipe, Draft::from_view)
    });
    let mut draft = use_signal(|| baseline.clone());
    let mut busy = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);
    let mut leave_prompt = use_confirm_slot::<(), ()>();
    // The enclosing dialog's close, when the open leave question was asked on
    // its behalf rather than for Back or Cancel (see `DialogCloseGuard`).
    let mut pending_close = use_signal(|| None::<Callback<()>>);
    let mut delete_prompt = use_confirm_slot::<(), ()>();
    let mounted = use_hook(|| Rc::new(Cell::new(true)));
    let drop_mounted = mounted.clone();
    use_drop(move || drop_mounted.set(false));

    let is_new = existing.is_none();
    let dirty = *draft.read() != baseline;
    let locked = busy();

    // Save: validate locally, then create or update. The editor's own Save
    // and the leave question's Save both run this; success reports through
    // `on_saved`, whose host leaves the editor, so the question needs no
    // separate "then leave" step. A refusal keeps the draft and the question.
    let save_base = base.clone();
    let save_mounted = mounted.clone();
    let save_existing = existing.clone();
    let save = use_callback(move |_: ()| {
        if busy() {
            return;
        }
        let current = draft();
        let name = current.name.trim().to_string();
        if save_existing.is_none()
            && let Err(reason) = check_recipe_name(&name)
        {
            error.set(Some(crate::local_validation_notice(reason.to_string())));
            return;
        }
        let request = match current.to_save() {
            Ok(request) => request,
            Err(reason) => {
                error.set(Some(crate::local_validation_notice(reason)));
                return;
            }
        };
        busy.set(true);
        error.set(None);
        // Declared here, in the click, so the save is registered even if
        // Settings closes before the spawned task is first polled.
        let sending: Request<RecipeView> = match &save_existing {
            None => Box::pin(create_recipe(
                &save_base,
                CreateRecipe {
                    name,
                    recipe: request,
                },
            )),
            Some(existing) => Box::pin(update_recipe(&save_base, &existing.name, request)),
        };
        let mounted = save_mounted.clone();
        dioxus::core::spawn_forever(async move {
            let result = sending.await;
            if !mounted.get() {
                return;
            }
            busy.set(false);
            match result {
                Ok(view) => {
                    // Read before `on_saved`, whose host unmounts this editor.
                    let close = pending_close.take();
                    on_saved.call(view);
                    if let Some(close) = close {
                        close.call(());
                    }
                }
                Err(reason) => error.set(Some(reason)),
            }
        });
    });

    // Leave (Back or Cancel): ask first only when there is something to lose.
    let leave_baseline = baseline.clone();
    let leave = use_callback(move |_: ()| {
        if busy() {
            return;
        }
        if *draft.peek() != leave_baseline {
            pending_close.set(None);
            leave_prompt.open((), ());
        } else {
            on_cancel.call(());
        }
    });

    // The enclosing dialog's Escape and close button: the same question as
    // Back, but Save and Discard then finish the close instead of returning
    // to the list. Asked even while a save runs (its buttons are disabled
    // until it ends) so a close never silently waits or drops the draft.
    let guard_baseline = baseline.clone();
    let hold_close = use_callback(move |close: Callback<()>| -> bool {
        if *draft.peek() == guard_baseline {
            return false;
        }
        pending_close.set(Some(close));
        leave_prompt.open((), ());
        true
    });
    let close_guard = try_use_context::<DialogCloseGuard>();
    use_hook(move || {
        if let Some(mut close_guard) = close_guard {
            close_guard.guard.set(Some(hold_close));
        }
    });
    use_drop(move || {
        if let Some(mut close_guard) = close_guard
            && *close_guard.guard.peek() == Some(hold_close)
        {
            close_guard.guard.set(None);
        }
    });

    let delete_base = base.clone();
    let delete_mounted = mounted.clone();
    let delete_name = existing.as_ref().map(|view| view.name.clone());
    let confirm_delete = use_callback(move |shown: crate::ops::ConfirmToken<()>| {
        if busy() || delete_prompt.take(&shown).is_none() {
            return;
        }
        let Some(name) = delete_name.clone() else {
            return;
        };
        busy.set(true);
        error.set(None);
        let sending: Request<()> = Box::pin(delete_recipe(&delete_base, &name));
        let mounted = delete_mounted.clone();
        dioxus::core::spawn_forever(async move {
            let result = sending.await;
            if !mounted.get() {
                return;
            }
            busy.set(false);
            match result {
                Ok(()) => match on_deleted {
                    Some(handler) => handler.call(name),
                    None => on_cancel.call(()),
                },
                Err(reason) => error.set(Some(reason)),
            }
        });
    });

    let leave_shown = leave_prompt.current();
    let delete_shown = delete_prompt.current();
    let software = draft.read().software;
    let tunnels = draft.read().tunnels.clone();
    let shown_name = existing
        .as_ref()
        .map(|view| view.name.clone())
        .unwrap_or_default();
    // A built snapshot is deleted with the recipe (the helm does it first),
    // so the confirmation says so: it is the part a user cannot get back.
    let delete_question = if existing
        .as_ref()
        .is_some_and(|view| view.snapshot.is_some())
    {
        format!("Delete recipe {shown_name} and its snapshot on Tensorlake? This can't be undone.")
    } else {
        format!("Delete recipe {shown_name}? This can't be undone.")
    };
    let title = match &existing {
        None => "New recipe".to_string(),
        Some(view) => format!("Recipe: {}", view.name),
    };

    let fields = rsx! {
        if !in_add_host {
            button { r#type: "button", class: "btn btn-neutral recipe-editor-back", disabled: locked,
                "data-tooltip": "Back to the recipe list; unsaved changes ask before leaving",
                onclick: move |_| leave.call(()), "‹ Recipes"
            }
        }
        if in_add_host {
            h3 { class: "recipe-editor-title", "{title}" }
        } else {
            h4 { class: "host-settings-section-title recipe-editor-title", tabindex: "-1",
                "{title}"
            }
        }
            p { class: "host-settings-help", "How Farhelm creates Tensorlake sandboxes for this recipe." }

            label { class: "checkout-root-label", span { "Name" }
                input { r#type: "text", class: "recipe-name font-mono", value: "{draft.read().name}",
                    readonly: !is_new || locked, maxlength: "32",
                    autocomplete: "off", autocorrect: "off", autocapitalize: "none", spellcheck: "false",
                    "data-tooltip": "The recipe's name; it can't be changed later",
                    oninput: move |event| { draft.write().name = event.value(); error.set(None); } }
            }
            p { class: "host-settings-help", "Can't be changed later. Hosts refer to their recipe by name. Lowercase letters, digits and hyphens." }

            div { class: "checkout-root-label", span { "Size" }
                div { class: "recipe-size",
                    label { class: "checkout-root-label",
                        span { "CPUs" }
                        input { r#type: "number", min: "1", step: "1", inputmode: "numeric", value: "{draft.read().cpus}", readonly: locked,
                            oninput: move |event| { draft.write().cpus = event.value(); error.set(None); } }
                        span { class: "recipe-hint", "1–4 on Usage Credits" }
                    }
                    label { class: "checkout-root-label",
                        span { "Memory, GB" }
                        input { r#type: "number", min: "1", step: "1", inputmode: "numeric", value: "{draft.read().memory_gb}", readonly: locked,
                            oninput: move |event| { draft.write().memory_gb = event.value(); error.set(None); } }
                        span { class: "recipe-hint", "1–8 per CPU on Usage Credits" }
                    }
                    label { class: "checkout-root-label",
                        span { "Disk, GB" }
                        input { r#type: "number", min: "1", step: "1", inputmode: "numeric", value: "{draft.read().disk_gb}", readonly: locked,
                            oninput: move |event| { draft.write().disk_gb = event.value(); error.set(None); } }
                        span { class: "recipe-hint", "10–100 on Usage Credits" }
                    }
                }
            }
            p { class: "host-settings-help", "Tensorlake decides what your plan allows when it creates a sandbox." }

            div { class: "checkout-root-label", span { "Software" }
                div { class: "recipe-source-cards", role: "group", aria_label: "Software",
                    button { r#type: "button", class: "recipe-source-card", disabled: locked,
                        aria_pressed: software == Software::Packages,
                        onclick: move |_| { draft.write().software = Software::Packages; error.set(None); },
                        span { class: "recipe-source-card-title", "Install packages" }
                        span { class: "recipe-source-card-help", "Farhelm builds a snapshot from Tensorlake's Ubuntu image with these packages, Claude Code and Codex." }
                    }
                    button { r#type: "button", class: "recipe-source-card", disabled: locked,
                        aria_pressed: software != Software::Packages,
                        onclick: move |_| {
                            if draft.peek().software == Software::Packages {
                                draft.write().software = Software::Image;
                            }
                            error.set(None);
                        },
                        span { class: "recipe-source-card-title", "Use an existing image or snapshot" }
                        span { class: "recipe-source-card-help", "Farhelm uses it as is and installs only its supervisor." }
                    }
                }
            }
            if software == Software::Packages {
                label { class: "checkout-root-label", span { "Ubuntu packages" }
                    textarea { class: "recipe-packages font-mono", rows: "4", value: "{draft.read().packages}", readonly: locked,
                        autocomplete: "off", autocorrect: "off", autocapitalize: "none", spellcheck: "false",
                        oninput: move |event| { draft.write().packages = event.value(); error.set(None); } }
                }
                p { class: "host-settings-help", "Separated by spaces or new lines. Changes take effect at the next snapshot build." }
            } else {
                div { class: "segmented-control", role: "group", aria_label: "Reuse",
                    button { r#type: "button", disabled: locked, aria_pressed: software == Software::Image,
                        onclick: move |_| { draft.write().software = Software::Image; error.set(None); }, "Image name" }
                    button { r#type: "button", disabled: locked, aria_pressed: software == Software::Snapshot,
                        onclick: move |_| { draft.write().software = Software::Snapshot; error.set(None); }, "Snapshot ID" }
                }
                if software == Software::Image {
                    label { class: "checkout-root-label", span { "Image" }
                        input { r#type: "text", class: "font-mono", value: "{draft.read().image}", readonly: locked,
                            autocomplete: "off", autocorrect: "off", autocapitalize: "none", spellcheck: "false",
                            oninput: move |event| { draft.write().image = event.value(); error.set(None); } }
                    }
                    p { class: "host-settings-help", "A Tensorlake image in your project; `tl sbx image ls` lists them." }
                } else {
                    label { class: "checkout-root-label", span { "Snapshot ID" }
                        input { r#type: "text", class: "font-mono", value: "{draft.read().snapshot_id}", readonly: locked,
                            autocomplete: "off", autocorrect: "off", autocapitalize: "none", spellcheck: "false",
                            oninput: move |event| { draft.write().snapshot_id = event.value(); error.set(None); } }
                    }
                    p { class: "host-settings-help", "A Tensorlake snapshot in your project; `tl sbx checkpoint ls` lists them." }
                }
                p { class: "host-settings-help recipe-systemd-note",
                    "Your image must have systemd installed, enabled, and running as the init system at startup (for example, images built from tensorlake/ubuntu-systemd). Farhelm prepares user services with sudo and installs its supervisor; nothing else. Installing Claude Code or Codex is up to the image."
                }
            }

            div { class: "checkout-root-label", span { "Tunnels to your side" }
                for (index, row) in tunnels.into_iter().enumerate() {
                    div { key: "{index}", class: "recipe-tunnel", role: "group", aria_label: "Tunnel {index + 1}",
                        input { r#type: "number", min: "1", max: "65535", inputmode: "numeric", class: "font-mono",
                            aria_label: "Sandbox port", placeholder: "Sandbox port", value: "{row.sandbox_port}", readonly: locked,
                            oninput: move |event| { if let Some(row) = draft.write().tunnels.get_mut(index) { row.sandbox_port = event.value(); } error.set(None); } }
                        span { class: "recipe-tunnel-arrow", aria_hidden: "true", "→" }
                        input { r#type: "text", class: "font-mono", aria_label: "Host", placeholder: "Host", value: "{row.host}", readonly: locked,
                            autocomplete: "off", autocorrect: "off", autocapitalize: "none", spellcheck: "false",
                            oninput: move |event| { if let Some(row) = draft.write().tunnels.get_mut(index) { row.host = event.value(); } error.set(None); } }
                        span { class: "recipe-tunnel-arrow", aria_hidden: "true", ":" }
                        input { r#type: "number", min: "1", max: "65535", inputmode: "numeric", class: "font-mono",
                            aria_label: "Port", placeholder: "Port", value: "{row.port}", readonly: locked,
                            oninput: move |event| { if let Some(row) = draft.write().tunnels.get_mut(index) { row.port = event.value(); } error.set(None); } }
                        button { r#type: "button", class: "btn btn-neutral", disabled: locked,
                            "data-tooltip": "Remove this tunnel from the recipe",
                            onclick: move |_| { let mut draft = draft.write(); if index < draft.tunnels.len() { draft.tunnels.remove(index); } },
                            "Remove"
                        }
                    }
                }
                button { r#type: "button", class: "btn btn-neutral recipe-add-tunnel", disabled: locked,
                    "data-tooltip": "Add a port in the sandbox that reaches a host and port on this helm's side",
                    onclick: move |_| draft.write().tunnels.push(TunnelDraft::default()),
                    "Add tunnel"
                }
            }
            p { class: "host-settings-help",
                "Programs in the sandbox reach 127.0.0.1 at the sandbox port, and Farhelm carries the connection to the host and port as seen from this helm's machine. Anything in the sandbox, agents included, can use a tunnel."
            }

            if let Some(reason) = (catalog.error)() {
                p { class: "checkout-root-error", role: "alert", "{reason}" }
                button { r#type: "button", class: "btn btn-neutral", disabled: locked,
                    "data-tooltip": "Load the named credentials again", onclick: move |_| catalog.reload.call(()), "Retry loading credentials" }
            } else if (catalog.rows)().is_none() {
                p { role: "status", "Loading credentials…" }
            }
            div { class: "checkout-root-label", span { "GitHub" }
                CredentialPicker {
                    label: "GitHub token", value: draft.read().github_token_credential.clone(),
                    kind: farhelm_proto::credentials::CredentialKind::GithubToken,
                    locked, credentials: (catalog.rows)(),
                    missing_notice: "The selected GitHub credential is unavailable. Choose another or None.",
                    on_change: move |value| { draft.write().github_token_credential = value; error.set(None); },
                }
                p { class: "host-settings-help",
                    "Sessions get this token as GH_TOKEN, and git uses it for github.com over HTTPS, including fresh checkouts of private repositories. Add or replace tokens in Settings → Credentials."
                }
            }
            div { class: "checkout-root-label", span { "Agents" }
                if software == Software::Packages {
                    p { class: "host-settings-help", "Claude Code and Codex are installed with their official installers when the snapshot is built." }
                }
                p { class: "host-settings-help",
                    "Optionally point either agent at an API endpoint. The base URL usually points at a tunnel's sandbox port, such as http://127.0.0.1:8080. Farhelm writes these into each host's agent settings when it sets the host up."
                }
                EndpointFields { agent: Agent::Claude, draft, locked, credentials: (catalog.rows)(), error }
                div { class: "recipe-endpoint", role: "group", aria_label: "Codex authentication",
                    span { "Codex" }
                    div { class: "segmented-control", role: "radiogroup", aria_label: "Codex authentication mode",
                        for (mode, label) in [(CodexMode::None, "None"), (CodexMode::Endpoint, "API endpoint"), (CodexMode::Subscription, "Subscription")] {
                            button { r#type: "button", role: "radio", disabled: locked,
                                aria_checked: draft.read().codex_mode == mode,
                                onclick: move |_| { draft.write().codex_mode = mode; error.set(None); }, "{label}" }
                        }
                    }
                    if draft.read().codex_mode == CodexMode::Endpoint {
                        EndpointFields { agent: Agent::Codex, draft, locked, credentials: (catalog.rows)(), error }
                    } else if draft.read().codex_mode == CodexMode::Subscription {
                        CredentialPicker {
                            label: "Codex login", value: draft.read().codex_login_credential.clone(),
                            kind: farhelm_proto::credentials::CredentialKind::CodexLogin,
                            locked, credentials: (catalog.rows)(),
                            missing_notice: "The selected Codex login is unavailable. Choose another login.",
                            on_change: move |value| { draft.write().codex_login_credential = value; error.set(None); },
                        }
                    }
                    p { class: "host-settings-help", "Subscription uses a Codex login from Settings → Credentials. Farhelm writes auth.json when that saved login changes and leaves Codex's refreshed file alone. Switching away leaves auth.json on the host; sign out in Codex to remove it." }
                }
                p { class: "host-settings-help",
                    "Add or replace endpoint tokens in Settings → Credentials. This recipe saves their names, not their values."
                }
            }

            if !is_new {
                p { class: "host-settings-help recipe-edit-note",
                    "Size and packages apply only to hosts created after you save (packages after the next snapshot build). Tunnels, endpoints and credential selections are read when a host connects or is set up, so those changes reach existing hosts at their next connection."
                }
            }

    };
    // Questions replace the ordinary actions in either layout. Keeping this
    // one branch preserves the same pending-close and confirmation ownership
    // when the answers move from the panel into Add host's pinned footer.
    let actions = rsx! {
            // Save can be used before the last field is visible. Its refusal
            // must stay beside those persistent answers, including while a
            // deferred close keeps the leave question open for another try.
            if let Some(reason) = error() {
                p { class: "checkout-root-error", role: "alert", "{reason}" }
            }
            if let Some(shown) = leave_shown.clone() {
                div { class: "templates-departure recipe-departure", role: "status",
                    // A close request can open this while the editor is out of
                    // view (another Settings section, or scrolled away); bring
                    // the whole answer surface into view, including its
                    // padding, before focusing Save. Scrolling just the inner
                    // question can leave its last border clipped at the body's
                    // fractional edge after a hidden panel is revealed.
                    onmounted: move |_| { document::eval("requestAnimationFrame(() => { const prompt = document.querySelector('.recipe-departure'); const surface = prompt?.closest('.recipe-editor-sticky-actions, .recipe-editor-footer') ?? prompt; surface?.scrollIntoView({block: 'nearest'}); prompt?.querySelector('.btn-primary')?.focus({preventScroll: true}); })"); },
                    p { "Save these changes before leaving?" }
                    DialogActions {
                        button { r#type: "button", class: "btn btn-neutral", disabled: locked,
                            "data-tooltip": "Stay here with the unsaved changes",
                            onclick: { let shown = shown.clone(); move |_| { if !busy() { leave_prompt.cancel_for(&shown); pending_close.set(None); } } },
                            "Keep editing"
                        }
                        button { r#type: "button", class: "btn btn-neutral", disabled: locked,
                            "data-tooltip": "Discard these changes and go back",
                            onclick: { let shown = shown.clone(); move |_| {
                                if !busy() && leave_prompt.take(&shown).is_some() {
                                    match pending_close.take() {
                                        Some(close) => close.call(()),
                                        None => on_cancel.call(()),
                                    }
                                }
                            } },
                            "Discard"
                        }
                        // Save leaves the question up while it runs and after a
                        // refusal (the reason shows above), so the user can
                        // answer again; success leaves through `on_saved`.
                        button { r#type: "button", class: "btn btn-primary", disabled: locked,
                            "data-tooltip": "Save these changes, then go back",
                            onclick: { let shown = shown.clone(); move |_| { if leave_prompt.is_current(&shown) { save.call(()); } } },
                            "Save"
                        }
                    }
                }
            } else if let Some(shown) = delete_shown.clone() {
                div { class: "templates-departure recipe-delete-confirm", role: "status",
                    p { "{delete_question}" }
                    DialogActions {
                        button { r#type: "button", class: "btn btn-neutral", disabled: locked,
                            onclick: { let shown = shown.clone(); move |_| delete_prompt.cancel_for(&shown) },
                            "Cancel"
                        }
                        button { r#type: "button", class: "btn btn-danger", disabled: locked,
                            onclick: { let shown = shown.clone(); move |_| confirm_delete.call(shown.clone()) },
                            "Delete"
                        }
                    }
                }
            } else {
                DialogActions { class: "recipe-editor-actions",
                    if allow_delete && !is_new {
                        button { r#type: "button", class: "btn btn-neutral recipe-delete", disabled: locked,
                            "data-tooltip": "Delete this recipe",
                            onclick: move |_| { if !busy() { delete_prompt.open((), ()); } },
                            "Delete"
                        }
                    }
                    button { r#type: "button", class: "btn btn-neutral", disabled: locked,
                        "data-tooltip": "Go back without saving; unsaved changes ask first",
                        onclick: move |_| leave.call(()), "Cancel"
                    }
                    button { r#type: "button", class: "btn btn-primary recipe-save", disabled: locked, aria_disabled: !dirty && !is_new,
                        "data-tooltip": "Save this recipe",
                        onclick: move |_| save.call(()),
                        if busy() { "Saving…" } else { "Save" }
                    }
                }
            }
    };
    if in_add_host {
        // The frame pins only direct body/footer siblings. A wrapper around
        // this fragment would put Save back inside the scrolling content.
        rsx! {
            div { class: "dialog-body host-settings-body",
                section { class: "host-settings-section recipe-editor", aria_label: "{title}",
                    onmounted: move |_| focus_editor(true),
                    {fields}
                }
            }
            DialogFooter { class: "host-dialog-footer recipe-editor-footer", {actions} }
        }
    } else {
        rsx! {
            section { class: "host-settings-section recipe-editor", aria_label: "{title}",
                onmounted: move |_| focus_editor(false),
                {fields}
                div { class: "recipe-editor-sticky-actions", {actions} }
            }
        }
    }
}

/// Which agent's endpoint an [`EndpointFields`] edits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Agent {
    Claude,
    Codex,
}

impl Agent {
    /// The agent's product name, as the form labels it.
    fn label(self) -> &'static str {
        match self {
            Agent::Claude => "Claude Code",
            Agent::Codex => "Codex",
        }
    }

    /// The draft's endpoint for this agent, to read.
    fn endpoint_ref(self, draft: &Draft) -> &EndpointDraft {
        match self {
            Agent::Claude => &draft.claude,
            Agent::Codex => &draft.codex,
        }
    }

    /// The draft's endpoint for this agent, to edit.
    fn endpoint(self, draft: &mut Draft) -> &mut EndpointDraft {
        match self {
            Agent::Claude => &mut draft.claude,
            Agent::Codex => &mut draft.codex,
        }
    }
}

/// Catalog selections share one unavailable-choice policy across consumers.
/// A disappeared selection remains visible until explicitly replaced; showing
/// None while submitting its old name would misrepresent the user's draft.
#[component]
fn CredentialPicker(
    label: String,
    value: String,
    kind: farhelm_proto::credentials::CredentialKind,
    locked: bool,
    credentials: Option<Vec<farhelm_proto::credentials::Credential>>,
    missing_notice: String,
    on_change: EventHandler<String>,
) -> Element {
    let loaded = credentials.is_some();
    let entries = credentials
        .unwrap_or_default()
        .into_iter()
        .filter(|row| row.kind == kind)
        .collect::<Vec<_>>();
    let missing = !value.is_empty() && !entries.iter().any(|row| row.name == value);
    rsx! {
        label { class: "checkout-root-label", span { "{label}" }
            // Option text changes independently of the control's identity.
            select { class: "recipe-credential font-mono", aria_label: "{label}",
                value: "{value}", disabled: locked || !loaded,
                onchange: move |event| on_change.call(event.value()),
                // Explicit selection survives replacement of a disappeared
                // option without the browser silently falling back to None.
                option { value: "", selected: value.is_empty(), "None" }
                if missing {
                    option { value: "{value}", selected: true, disabled: true,
                        if loaded { "{value} (unavailable)" } else { "{value}" } }
                }
                for row in &entries { option { key: "{row.name}", value: "{row.name}",
                    selected: row.name == value, "{row.name}" } }
            }
        }
        if loaded && missing { p { class: "checkout-root-error", role: "alert", "{missing_notice}" } }
    }
}

/// An endpoint keeps its own URL draft while reusing the metadata-only picker.
/// The store remains authoritative for admitting the chosen name and kind.
#[component]
fn EndpointFields(
    agent: Agent,
    draft: Signal<Draft>,
    locked: bool,
    credentials: Option<Vec<farhelm_proto::credentials::Credential>>,
    error: Signal<Option<String>>,
) -> Element {
    let mut draft = draft;
    let mut error = error;
    let current = agent.endpoint_ref(&draft.read()).clone();
    let label = agent.label();
    rsx! {
        div { class: "recipe-endpoint", role: "group", aria_label: "{label} endpoint",
            label { class: "checkout-root-label", span { "{label} base URL" }
                input { r#type: "url", class: "font-mono", value: "{current.base_url}", readonly: locked,
                    placeholder: "http://127.0.0.1:8080",
                    autocomplete: "off", autocorrect: "off", autocapitalize: "none", spellcheck: "false",
                    oninput: move |event| { agent.endpoint(&mut draft.write()).base_url = event.value(); error.set(None); } }
            }
            CredentialPicker {
                label: format!("{label} endpoint token"), value: current.token_credential,
                kind: farhelm_proto::credentials::CredentialKind::EndpointToken,
                locked, credentials,
                missing_notice: "The selected endpoint credential is unavailable. Choose another or None.",
                on_change: move |value| { agent.endpoint(&mut draft.write()).token_credential = value; error.set(None); },
            }
        }
    }
}

#[cfg(test)]
#[path = "recipe_editor_tests.rs"]
mod tests;
