//! The Tensorlake recipe editor: one recipe's form, as a body rather than a
//! dialog.
//!
//! It is a body so two hosts can embed it without nesting modals: Settings →
//! Tensorlake swaps its recipe list for it, and the add-host dialog will open
//! it in place for "New recipe…" (the add-host change; there `allow_delete`
//! stays off). The editor owns everything about one editing session: the
//! draft and its baseline, local validation, the save or delete request, and
//! the "Save these changes before leaving?" question when Back or Cancel would
//! drop unsaved edits. The host only decides what happens after it reports
//! saved, cancelled or deleted.
//!
//! Requests follow the explicit-save pattern of the ntfy form: they are
//! declared in the click handler and spawned with `spawn_forever`, so closing
//! Settings does not retract an accepted save, and they touch component state
//! only while it is still mounted. Saved endpoint tokens never reach the
//! form; a blank token field keeps the saved one under the helm's origin
//! rule, which the form explains before saving.
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
use crate::modal_dialog::DialogActions;
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

/// One agent endpoint as typed. `token` is a replacement only (the saved one
/// is never loaded); `clear` asks the helm to drop the saved token.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct EndpointDraft {
    base_url: String,
    token: String,
    clear: bool,
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
/// opened) is what "unsaved changes" means, so a typed replacement token
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
            ..EndpointDraft::default()
        };
        draft.claude = endpoint(&view.claude);
        draft.codex = endpoint(&view.codex);
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
        let save = RecipeSave {
            size,
            source,
            claude: endpoint_save("Claude Code", &self.claude)?,
            codex: endpoint_save("Codex", &self.codex)?,
            tunnels,
        };
        check_recipe(&save)?;
        Ok(save)
    }
}

/// The save shape of one endpoint draft.
///
/// A blank base URL means "no endpoint", which also drops a saved token; a
/// replacement token without a base URL is a mistake worth saying so about.
/// With a base URL, `clear` sends an empty token (drop it), a typed token
/// replaces it, and a blank field sends none (the helm keeps the saved one if
/// the origin is unchanged).
fn endpoint_save(agent: &str, draft: &EndpointDraft) -> Result<Option<EndpointSave>, String> {
    let base_url = draft.base_url.trim();
    if base_url.is_empty() {
        if !draft.token.is_empty() && !draft.clear {
            return Err(format!(
                "{agent}: enter a base URL, or remove the token you typed"
            ));
        }
        return Ok(None);
    }
    let token = if draft.clear {
        Some(String::new())
    } else if draft.token.is_empty() {
        None
    } else {
        Some(draft.token.clone())
    };
    Ok(Some(EndpointSave {
        base_url: base_url.to_string(),
        token,
    }))
}

/// What happens to a saved token when this draft is saved, for the token
/// field's placeholder and the hint below it. Mirrors the helm's rule, but
/// the helm decides: a stale form cannot keep a token by this prediction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TokenFate {
    /// No token is saved for this endpoint.
    None,
    /// The saved token stays.
    Kept,
    /// A typed token replaces the saved one.
    Replaced,
    /// The saved token is dropped: cleared, origin changed, or the endpoint
    /// removed.
    Dropped,
}

/// Predict [`TokenFate`] for one endpoint from what is saved and the draft.
fn token_fate(saved: Option<&EndpointView>, draft: &EndpointDraft) -> TokenFate {
    let Some(saved) = saved.filter(|saved| saved.token_set) else {
        return TokenFate::None;
    };
    if draft.clear || draft.base_url.trim().is_empty() {
        TokenFate::Dropped
    } else if !draft.token.is_empty() {
        TokenFate::Replaced
    } else if crate::ntfy_settings::server_origin_changed(&saved.base_url, draft.base_url.trim()) {
        TokenFate::Dropped
    } else {
        TokenFate::Kept
    }
}

// ---------------------------------------------------------------------------
// The component
// ---------------------------------------------------------------------------

/// A dialog's close request, held back while an embedded editor has unsaved
/// changes.
///
/// The editor lives inside another modal (Settings today, the add-host dialog
/// later), and that modal's Escape and close button would otherwise unmount it
/// and drop the draft without the "Save these changes before leaving?"
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

/// One recipe's editor. `existing: None` edits a new recipe.
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
    #[props(default)] allow_delete: bool,
    #[props(default)] on_deleted: Option<EventHandler<String>>,
) -> Element {
    let base = use_context::<ApiBase>().0;
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
    let saved_claude = existing.as_ref().and_then(|view| view.claude.clone());
    let saved_codex = existing.as_ref().and_then(|view| view.codex.clone());
    let software = draft.read().software;
    let tunnels = draft.read().tunnels.clone();
    let shown_name = existing
        .as_ref()
        .map(|view| view.name.clone())
        .unwrap_or_default();
    let title = match &existing {
        None => "New recipe".to_string(),
        Some(view) => format!("Recipe: {}", view.name),
    };

    rsx! {
        section { class: "host-settings-section recipe-editor", aria_label: "{title}",
            button { r#type: "button", class: "btn btn-neutral recipe-editor-back", disabled: locked,
                "data-tooltip": "Back to the recipe list; unsaved changes ask before leaving",
                onclick: move |_| leave.call(()), "‹ Recipes"
            }
            h4 { class: "host-settings-section-title recipe-editor-title", tabindex: "-1",
                // The click that opened the editor removed its own button;
                // without this, keyboard focus would fall back to the body.
                onmounted: move |event| { spawn(async move { let _ = event.set_focus(true).await; }); },
                "{title}"
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

            div { class: "checkout-root-label", span { "Agents" }
                if software == Software::Packages {
                    p { class: "host-settings-help", "Claude Code and Codex are installed with their official installers when the snapshot is built." }
                }
                p { class: "host-settings-help",
                    "Optionally point either agent at an API endpoint. The base URL usually points at a tunnel's sandbox port, such as http://127.0.0.1:8080. Farhelm writes these into each host's agent settings when it sets the host up."
                }
                EndpointFields { agent: Agent::Claude, draft, locked, saved: saved_claude.clone(), error }
                EndpointFields { agent: Agent::Codex, draft, locked, saved: saved_codex.clone(), error }
                p { class: "host-settings-help",
                    "Tokens are stored unencrypted in the helm's state on this machine. Farhelm never shows them again."
                }
            }

            if !is_new {
                p { class: "host-settings-help recipe-edit-note",
                    "Size and packages apply only to hosts created after you save (packages after the next snapshot build). Tunnels and endpoints are read when a host connects or is set up, so tunnel changes reach existing hosts at their next connection."
                }
            }

            if let Some(reason) = error() {
                p { class: "checkout-root-error", role: "alert", "{reason}" }
            }
            if let Some(shown) = leave_shown.clone() {
                div { class: "templates-departure recipe-departure", role: "status",
                    // A close request can open this while the editor is out of
                    // view (another Settings section, or scrolled away); bring
                    // the question and its Save into view and focus.
                    onmounted: move |_| { document::eval("requestAnimationFrame(() => { const prompt = document.querySelector('.recipe-departure'); prompt?.scrollIntoView({block: 'nearest'}); prompt?.querySelector('.btn-primary')?.focus({preventScroll: true}); })"); },
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
                    p { "Delete recipe {shown_name}? This can't be undone." }
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

    /// What the agent's own configuration calls the token: Claude Code's
    /// `ANTHROPIC_AUTH_TOKEN`, Codex's `experimental_bearer_token`.
    fn token_label(self) -> &'static str {
        match self {
            Agent::Claude => "Auth token",
            Agent::Codex => "Bearer token",
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

/// One agent's endpoint fields: base URL, a write-only token, and a clear
/// switch when a token is saved.
///
/// The token field never shows the saved token; its placeholder says what
/// saving will do to it ([`token_fate`]), and a hint appears when an origin
/// change would drop it, the way the ntfy form explains its own token.
/// `new-password` rather than `off`: browsers ignore `off` for password
/// fields and could otherwise fill the helm's own saved web token here, which
/// would then be sent to the endpoint.
#[component]
fn EndpointFields(
    agent: Agent,
    draft: Signal<Draft>,
    locked: bool,
    saved: Option<EndpointView>,
    error: Signal<Option<String>>,
) -> Element {
    let mut draft = draft;
    let mut error = error;
    let current = agent.endpoint_ref(&draft.read()).clone();
    let fate = token_fate(saved.as_ref(), &current);
    let token_set = saved.as_ref().is_some_and(|saved| saved.token_set);
    let label = agent.label();
    let token_label = agent.token_label();
    let placeholder = match fate {
        TokenFate::None => "Optional",
        TokenFate::Kept | TokenFate::Replaced => "Token saved; leave blank to keep it",
        TokenFate::Dropped => "Saved token will be cleared",
    };
    rsx! {
        div { class: "recipe-endpoint", role: "group", aria_label: "{label} endpoint",
            label { class: "checkout-root-label", span { "{label} base URL" }
                input { r#type: "url", class: "font-mono", value: "{current.base_url}", readonly: locked,
                    placeholder: "http://127.0.0.1:8080",
                    autocomplete: "off", autocorrect: "off", autocapitalize: "none", spellcheck: "false",
                    oninput: move |event| { agent.endpoint(&mut draft.write()).base_url = event.value(); error.set(None); } }
            }
            label { class: "checkout-root-label", span { "{label} {token_label.to_lowercase()}" }
                input { r#type: "password", class: "recipe-token", value: "{current.token}", readonly: locked || current.clear,
                    autocomplete: "new-password", autocorrect: "off", autocapitalize: "none", spellcheck: "false",
                    placeholder,
                    oninput: move |event| { agent.endpoint(&mut draft.write()).token = event.value(); error.set(None); } }
            }
            if token_set && fate == TokenFate::Dropped && !current.clear && !current.base_url.trim().is_empty() {
                p { class: "host-settings-help recipe-token-origin-hint", "Changing the server clears the saved token when you save" }
            }
            if token_set {
                label { class: "app-settings-choice setting-row",
                    span { "Clear saved {token_label.to_lowercase()}" }
                    input { r#type: "checkbox", class: "control-switch", checked: current.clear, disabled: locked,
                        onchange: move |event| { agent.endpoint(&mut draft.write()).clear = event.checked(); error.set(None); } }
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "recipe_editor_tests.rs"]
mod tests;
