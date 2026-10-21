//! The modal boundary owns focus installation and release-before-return.
//!
//! A caller still owns its backdrop, contents and operation state. The shell
//! renders the existing dialog root itself, so extracting focus behavior does
//! not add a wrapper or change selectors. This is also the seam where a later
//! native `<dialog>` conversion can replace the boundary without moving each
//! dialog's contents or request ownership.
//!
//! Closing is explicit: the caller decides whether its live operation permits
//! dismissal and which focus destination belongs to that close reason. DOM
//! removal only releases leftover isolation; it never returns focus, since a
//! successful handoff can intentionally give the keyboard to another surface.

use dioxus::prelude::*;

/// Address the same modal from its shell and from its surviving state owner.
///
/// Async success can close a dialog from its parent, so release cannot depend
/// on a child hook remaining mounted. A static selector may carry a numeric
/// opening fence; it still resolves the first root before checking identity.
/// This handle does not arbitrate multiple dialogs or queue them.
#[derive(Clone, Copy, PartialEq)]
pub(crate) struct DialogHandle {
    selector: &'static str,
    generation: Option<(&'static str, u64)>,
}

impl DialogHandle {
    /// Bind focus operations to the root whose lifecycle the caller owns.
    pub(crate) const fn new(selector: &'static str) -> Self {
        Self {
            selector,
            generation: None,
        }
    }

    /// Fence deferred DOM work against a later opening at the same selector.
    ///
    /// Resolve the first root before comparing its numeric generation. Filtering
    /// the selector instead could skip a replacement root and find an older one.
    pub(crate) const fn for_generation(mut self, attribute: &'static str, generation: u64) -> Self {
        self.generation = Some((attribute, generation));
        self
    }

    /// Compare only the already resolved root, before touching its focus state.
    fn generation_guard(self) -> String {
        self.generation
            .map_or(String::new(), |(attribute, generation)| {
                let attribute =
                    serde_json::to_string(attribute).expect("a string always serializes");
                format!(" || dialog.getAttribute({attribute}) !== '{generation}'")
            })
    }

    /// Apply an existing modal operation only to the expected mounted opening.
    fn bound_script(self, body: &str) -> String {
        if self.generation.is_none() {
            return body.to_owned();
        }
        let selector = serde_json::to_string(self.selector).expect("a string always serializes");
        let generation_guard = self.generation_guard();
        format!(
            "(() => {{ const dialog = document.querySelector({selector}); if (!dialog{generation_guard}) return; {body} }})();"
        )
    }

    /// Release only this opening; an absent or replacement root owns no release.
    fn release_script(self) -> String {
        self.bound_script(&crate::modal_isolation::release_js(self.selector))
    }

    /// Release isolation now and return focus after the closing render.
    ///
    /// The supplied body chooses the destination for this close reason. It
    /// may also resume a still-mounted parent's isolation around that action.
    /// Release and frame scheduling live here so no caller can focus its
    /// still-inert opener first. A missing destination retains the caller's
    /// existing no-op behavior.
    pub(crate) fn close(self, return_focus_body: &str) {
        document::eval(&format!(
            "{} requestAnimationFrame(() => {{ {return_focus_body} }});",
            self.release_script(),
        ));
    }

    /// Release for a modal handoff without scheduling a competing focus return.
    ///
    /// The next modal owns its initial focus. Ordinary DOM-removal cleanup is
    /// still the fallback for success paths that do not explicitly close.
    pub(crate) fn release(self) {
        document::eval(&self.release_script());
    }

    /// Return immediately after removal, unless a newer control already has focus.
    ///
    /// The surviving owner decides whether this return is owed and submits it
    /// from its existing post-render effect. A missing old root must not suppress
    /// that destination; generation fences only the release. Unlike `close`, no
    /// frame is scheduled. The body chooses only the domain destination.
    /// An isolated caller must still release before requesting removal, even
    /// when no return is owed; this post-render release cannot supply that order.
    pub(crate) fn close_immediately_if_document_focused(self, return_focus_body: &str) {
        document::eval(&format!(
            r#"{} (() => {{
                const active = document.activeElement;
                if (active !== document.body && active !== document.documentElement) return;
                {return_focus_body}
            }})();"#,
            self.release_script(),
        ));
    }

    /// Release isolation and wait for this captured root to leave the document.
    ///
    /// A terminal's focus veto also checks modal DOM, so releasing inert alone
    /// cannot complete a navigation handoff. The surviving owner must await
    /// this operation from its own task after requesting the closing render;
    /// a dialog-owned task would be cancelled by the removal it waits for.
    /// Release and node capture share one eval, and removal is observed on
    /// that exact node rather than inferred from a frame or a later selector.
    /// As with the existing handoff, an eval failure does not block the owner.
    pub(crate) async fn close_and_wait(self) {
        let selector = serde_json::to_string(self.selector).expect("a string always serializes");
        let generation_match = self
            .generation
            .map_or(String::new(), |(attribute, generation)| {
                let attribute =
                    serde_json::to_string(attribute).expect("a string always serializes");
                format!(" && dialog.getAttribute({attribute}) === '{generation}'")
            });
        let js = format!(
            r#"{}
        const dialog = document.querySelector({selector});
        if (dialog{generation_match}) await new Promise(resolve => {{
            const observer = new MutationObserver(() => {{
                if (!dialog.isConnected) {{ observer.disconnect(); resolve(); }}
            }});
            observer.observe(document.body, {{ childList: true, subtree: true }});
        }});
        return true;"#,
            self.release_script()
        );
        let _ = document::eval(&js).join::<bool>().await;
    }

    /// Resume a mounted parent's isolation without repeating its initial focus.
    ///
    /// Nested help and consent return to parents whose gap handlers and focus
    /// memory remain installed. Their callers place this generated operation
    /// before or after the destination action in the shared close frame,
    /// preserving each handoff's existing order without submitting another eval.
    pub(crate) fn isolation_script(self, escape_target: Option<&str>) -> String {
        self.bound_script(&crate::modal_isolation::install_js(
            self.selector,
            escape_target,
        ))
    }

    /// Install the opening's focus policy without repeating it on rerenders.
    fn install(self, focus: &DialogFocus) {
        install_focus(self, focus);
    }
}

/// An opener captured before a modal takes focus, consumed after admission.
///
/// The caller owns capture and the decision whether this close owes a return.
/// A successful navigation can give focus to a new terminal or another modal;
/// spending the old capture still matters when no restoration is owed.
#[derive(Clone, Copy)]
pub(crate) struct SavedFocus {
    window_key: &'static str,
}

impl SavedFocus {
    /// Address the existing window slot without capturing or moving focus.
    pub(crate) const fn new(window_key: &'static str) -> Self {
        Self { window_key }
    }

    /// Consume the opener now and focus it only when this close owes a return.
    ///
    /// Call after isolation release and the caller's navigation decision.
    /// Consumption is unconditional, including a detached or unused opener;
    /// missing destinations have no fallback and no delayed return is queued.
    pub(crate) fn restore(self, restore: bool) {
        let key = serde_json::to_string(self.window_key).expect("a string always serializes");
        document::eval(&format!(
            r#"
        const previous = window[{key}];
        delete window[{key}];
        if ({restore} && previous?.isConnected) previous.focus({{ preventScroll: true }});
    "#
        ));
    }
}

/// Select a domain focus destination without owning modal release or timing.
///
/// Explicit handoffs resolve both selectors from document in priority order;
/// the installer uses the same rule relative to its mounted dialog. A missing
/// destination does nothing, preserving handoffs whose opener vanished. The
/// target can run after installation, on offer arrival, or as the destination-
/// only body of an already-approved close.
#[derive(Clone, PartialEq)]
pub(crate) struct FocusTarget {
    selector: String,
    fallback: Option<String>,
    quiet: bool,
}

impl FocusTarget {
    /// Describe the preferred target and optional fallback without moving focus.
    pub(crate) fn new(selector: impl Into<String>, fallback: Option<&str>, quiet: bool) -> Self {
        Self {
            selector: selector.into(),
            fallback: fallback.map(str::to_owned),
            quiet,
        }
    }

    /// Generate only the destination action; close owns release and scheduling.
    pub(crate) fn script(&self) -> String {
        target_js("document", self, false)
    }

    /// Perform a caller-owned handoff now, without reinstalling modal behavior.
    pub(crate) fn focus(&self) {
        document::eval(&self.script());
    }
}

/// Share target preference and quiet focus while retaining each lookup scope.
///
/// `scope` is an internal JS receiver, either the mounted dialog or document.
/// Remembered initial focus also falls back to its root; explicit handoffs do
/// not. The self-contained conditional never returns from an enclosing trap
/// installer when its initial target is absent, since the trap is still needed.
fn target_js(scope: &str, target: &FocusTarget, root_fallback: bool) -> String {
    let preferred = serde_json::to_string(&target.selector).expect("a string always serializes");
    let fallback = target.fallback.as_ref().map_or(String::new(), |selector| {
        let selector = serde_json::to_string(selector).expect("a string always serializes");
        format!(" ?? {scope}.querySelector({selector})")
    });
    let root = if root_fallback { " ?? dialog" } else { "" };
    let quiet = target.quiet;
    format!(
        r#"(() => {{
            const target = {scope}.querySelector({preferred}){fallback}{root};
            if (target) {{
                // Focus safety does not depend on the tooltip asset finishing load.
                // A later installation observes new events, not this past focus.
                if ({quiet} && window.farhelmTooltip?.focusWithoutTooltip) window.farhelmTooltip.focusWithoutTooltip(target);
                else target.focus({{ preventScroll: true }});
            }}
        }})();"#,
    )
}

/// Keep the existing trap's first focus and listener behind the same guard.
///
/// Boundary wrapping deliberately uses native focus scrolling: a contained
/// control outside the dialog's scroll viewport must become visible too.
/// A handoff without a trap still uses this guard to consume its capture once.
fn trap_js(handle: DialogHandle, trap: Option<TabTrap>, initial_focus: &str) -> String {
    let dialog = serde_json::to_string(handle.selector).expect("a string always serializes");
    let generation_guard = handle.generation_guard();
    let listener = trap.map_or(String::new(), |trap| {
        let selector = serde_json::to_string(trap.selector).expect("a string always serializes");
        let empty_to_root = trap.empty_to_root;
        let reverse_from_root = trap.reverse_from_root;
        format!(
            r#"const focusable = () => [...dialog.querySelectorAll({selector})]
                    .filter(node => !node.hidden && node.getClientRects().length);
                dialog.addEventListener('keydown', event => {{
                    if (event.key !== 'Tab') return;
                    const nodes = focusable();
                    if (!nodes.length) {{
                        if ({empty_to_root}) {{
                            event.preventDefault();
                            dialog.focus({{ preventScroll: true }});
                        }}
                        return;
                    }}
                    const first = nodes[0], last = nodes[nodes.length - 1];
                    const atStart = document.activeElement === first
                        || ({reverse_from_root} && document.activeElement === dialog);
                    if (event.shiftKey ? atStart : document.activeElement === last) {{
                        event.preventDefault();
                        (event.shiftKey ? last : first).focus();
                    }}
                }});"#,
        )
    });
    format!(
        r#"(() => {{
                const dialog = document.querySelector({dialog});
                if (!dialog{generation_guard} || dialog.__farhelmDialogTrap) return;
                dialog.__farhelmDialogTrap = true;
                {initial_focus}
                {listener}
            }})();"#,
    )
}

/// Preserve an opening's admission decision and consume its exact saved opener.
///
/// Sample once and compare before clearing the capture, even when unconditional
/// naming focus wins or an unrelated control makes ordinary focus yield. The
/// caller places this body behind the generation and once-per-node guards.
fn handoff_js(
    selector: &str,
    select_all: bool,
    captured_opener: Option<&str>,
    admission: &FocusAdmission,
) -> String {
    let target = serde_json::to_string(selector).expect("a string always serializes");
    let capture = captured_opener.map_or_else(
        || "const capturedOpener = false;".to_owned(),
        |key| {
            let key = serde_json::to_string(key).expect("a string always serializes");
            format!("const capturedOpener = active === window[{key}]; window[{key}] = null;")
        },
    );
    let admitted = match admission {
        FocusAdmission::Always => "true".to_owned(),
        FocusAdmission::DocumentOrOpener { selector } => {
            let selector = serde_json::to_string(selector).expect("a string always serializes");
            format!(
                "!active || active === document.body || active.matches({selector}) || capturedOpener"
            )
        }
    };
    let select = if select_all { "input?.select();" } else { "" };
    format!(
        r#"const active = document.activeElement;
                {capture}
                if ({admitted}) {{
                    const input = dialog.querySelector({target});
                    input?.focus({{ preventScroll: true }});
                    {select}
                }}"#,
    )
}

/// Install the shell's opening policy in its existing eval order.
///
/// Initial focus precedes isolation, which otherwise blurs an inerted opener.
/// Remembered dialogs keep their existing two eval submissions: focus and
/// isolation first, gap/Enter second. Direct trapped dialogs keep gap/Enter
/// before isolation in one eval. These are mount contracts, not effects that
/// should run again whenever a dialog's state changes.
fn install_focus(handle: DialogHandle, focus: &DialogFocus) {
    let dialog_selector = handle.selector;
    let dialog = serde_json::to_string(dialog_selector).expect("a string always serializes");
    let generation_guard = handle.generation_guard();
    let setup = match &focus.initial_focus {
        InitialFocus::Target(selector) => {
            let target = FocusTarget::new(selector.clone(), None, false);
            let initial = target_js("dialog", &target, false);
            if let Some(trap) = focus.trap {
                trap_js(handle, Some(trap), &initial)
            } else {
                format!(
                    "(() => {{ const dialog = document.querySelector({dialog}); if (!dialog{generation_guard}) return; {initial} }})();"
                )
            }
        }
        InitialFocus::Remembered { selector, quiet } => {
            // Disabled confirming answers retain a separate Cancel/root fallback;
            // the independent Escape selector still chooses its ordinary handler.
            let target = FocusTarget::new(
                selector.clone(),
                Some(".host-settings-close, .uninstall-cancel"),
                *quiet,
            );
            let initial_focus = target_js("dialog", &target, true);
            let mut setup = format!(
                r#"(() => {{
                    const dialog = document.querySelector({dialog});
                    if (!dialog{generation_guard}) return;
                    if (!dialog.__farhelmFocusMemory) {{
                        dialog.__farhelmFocusMemory = true;
                        dialog.addEventListener('focusin', (event) => {{
                            if (event.target !== dialog) dialog.__farhelmLastFocus = event.target;
                        }});
                    }}
                    {initial_focus}
                }})();"#,
            );
            if let Some(trap) = focus.trap {
                setup.push_str(&trap_js(handle, Some(trap), ""));
            }
            setup
        }
        InitialFocus::Handoff {
            selector,
            select_all,
            captured_opener,
            admission,
        } => trap_js(
            handle,
            focus.trap,
            &handoff_js(selector, *select_all, *captured_opener, admission),
        ),
    };
    let action = focus.main_action.as_deref().map_or(
        crate::dialog_focus::ContainerEnter::None,
        crate::dialog_focus::ContainerEnter::Click,
    );
    let gap = if focus.gap {
        handle.bound_script(&crate::dialog_focus::install_js(dialog_selector, action))
    } else {
        String::new()
    };
    let isolate = if focus.isolation {
        handle.isolation_script(focus.escape_target.as_deref())
    } else {
        String::new()
    };
    match &focus.initial_focus {
        InitialFocus::Target(_) | InitialFocus::Handoff { .. } => {
            document::eval(&format!("{setup}\n{gap}\n{isolate}"));
        }
        InitialFocus::Remembered { .. } => {
            document::eval(&format!("{setup} {isolate}"));
            document::eval(&gap);
        }
    };
}

/// Preserve the dialog's actual Tab stops and its container fallback policy.
///
/// The selector must exclude arrow-owned option rows when native Tab skips
/// them. A root parked by the isolation safety net can count as the reverse
/// boundary; a render with no stops can likewise keep Tab on that root.
#[derive(Clone, Copy, PartialEq)]
pub(crate) struct TabTrap {
    pub(crate) selector: &'static str,
    pub(crate) empty_to_root: bool,
    pub(crate) reverse_from_root: bool,
}

/// Preserve each dialog family's initial-focus and installation-order contract.
///
/// The remembered policy keeps the host fallback and recovery memory together;
/// the direct target retains Restart with's focus inside the trap's guard.
#[derive(Clone, PartialEq)]
pub(crate) enum InitialFocus {
    /// Focus only the named descendant, without fallback or focus memory.
    Target(String),
    /// Remember focus before moving it, with the host family's Cancel/root fallback.
    ///
    /// Memory installation is idempotent; initial focus still runs on every
    /// explicit installation, as it did for the host installers. Quiet focus
    /// suppresses the initial tooltip without depending on its asset loading.
    Remembered { selector: String, quiet: bool },
    /// Hand off once, yielding to newer focus unless the opening owns all sources.
    ///
    /// A supplied capture is compared and nulled even when focus is denied.
    /// An admitted naming handoff can select its title while ordinary editing
    /// leaves the native selection untouched.
    Handoff {
        selector: String,
        select_all: bool,
        captured_opener: Option<&'static str>,
        admission: FocusAdmission,
    },
}

/// Which current focus sources an opening is allowed to replace.
#[derive(Clone, PartialEq)]
pub(crate) enum FocusAdmission {
    /// The opening owns one handoff regardless of the current active control.
    Always,
    /// Yield to later outside focus, except for the caller's named or saved opener.
    ///
    /// Missing focus and body are admitted; documentElement is deliberately
    /// excluded here, unlike the post-removal return rule.
    DocumentOrOpener { selector: String },
}

/// Keep mount targets separate from live cancellation and request ownership.
///
/// Missing trap and main action retain a dialog's current native Tab and Enter
/// behavior; initial focus never implies that the same action handles Escape.
/// Isolation and gap retention are independent: neither follows from a trap
/// or from the absence of a container action. Existing constructors enable both.
#[derive(Clone, PartialEq)]
pub(crate) struct DialogFocus {
    pub(crate) initial_focus: InitialFocus,
    pub(crate) trap: Option<TabTrap>,
    pub(crate) escape_target: Option<String>,
    pub(crate) main_action: Option<String>,
    /// Preserve existing page isolation independently of Tab trapping.
    pub(crate) isolation: bool,
    /// Preserve gap-click retention independently of a container Enter action.
    pub(crate) gap: bool,
}

impl DialogFocus {
    /// Preserve the host-family policy while callers move onto the shared root.
    ///
    /// These dialogs already isolate, but do not yet trap Tab. Keeping that
    /// absence explicit separates the structural conversion from the later fix.
    pub(crate) fn remembered(
        initial_focus: &str,
        escape_target: Option<&str>,
        quiet: bool,
        main_action: Option<&str>,
    ) -> Self {
        Self {
            initial_focus: InitialFocus::Remembered {
                selector: initial_focus.to_owned(),
                quiet,
            },
            trap: None,
            escape_target: escape_target.map(str::to_owned),
            main_action: main_action.map(str::to_owned),
            isolation: true,
            gap: true,
        }
    }
}

/// Keep a form dialog's native submit boundary on the modal root itself.
///
/// The shell adds no wrapper: controls and form submission retain the same
/// ancestor and CSS relationships as the original div or form root.
#[derive(Clone, Copy, Default, PartialEq)]
pub(crate) enum DialogRoot {
    /// Keep ordinary dialogs outside the browser's native submit behavior.
    #[default]
    Div,
    /// Let destination-field Enter and explicit submit buttons submit this root.
    Form,
}

/// Render one existing modal root and own its mount-time focus boundary.
///
/// Attributes remain caller-supplied so markup, labels and root focusability
/// stay unchanged. Escape delegates to the caller's live state and layered
/// cancellation rules; other keys keep their existing root handler. Child
/// focus handoffs remain independent, and ordinary rerenders never repeat
/// initial focus over a later child question. The optional mounted callback
/// runs after the shared installation has been submitted; it preserves domain
/// work such as Settings' delayed trust-section scroll, not a second installer.
/// Movement callbacks attach to this same root only when supplied, so dialogs
/// without resize behavior do not acquire an otherwise unused resize observer.
/// Click handling likewise stays on the root when a caller separates its
/// controls from a dismissible backdrop; absent handlers remain absent.
/// An optional second focus target preserves setup's existing root-first
/// isolation, then quiet confirmation handoff, before domain mounted work.
/// A nested consent may first release its still-mounted parent; this remains
/// a separate eval before initial focus, rather than another mount callback.
/// Focus ownership listeners also attach to the actual root: a wrapper would
/// change the boundary that records whether an async close still owns focus.
#[component]
pub(crate) fn ModalDialog(
    handle: DialogHandle,
    focus: DialogFocus,
    on_escape: EventHandler<KeyboardEvent>,
    #[props(default)] root: DialogRoot,
    #[props(default)] before_install_release: Option<DialogHandle>,
    #[props(default)] after_install_focus: Option<FocusTarget>,
    #[props(default)] onkeydown: Option<EventHandler<KeyboardEvent>>,
    #[props(default)] onmounted: Option<EventHandler<MountedEvent>>,
    #[props(default)] onscroll: Option<EventHandler<ScrollEvent>>,
    #[props(default)] onresize: Option<EventHandler<ResizeEvent>>,
    #[props(default)] onsubmit: Option<EventHandler<FormEvent>>,
    #[props(default)] onclick: Option<EventHandler<MouseEvent>>,
    #[props(default)] onfocusin: Option<EventHandler<FocusEvent>>,
    #[props(default)] onfocusout: Option<EventHandler<FocusEvent>>,
    #[props(extends = GlobalAttributes)] attributes: Vec<Attribute>,
    children: Element,
) -> Element {
    // Native roots share one lifecycle boundary. Keeping their handlers here
    // avoids giving forms a second copy of focus installation or Escape rules.
    let mut root_attributes = vec![
        dioxus::html::onmounted(move |event: MountedEvent| {
            if let Some(parent) = before_install_release {
                parent.release();
            }
            handle.install(&focus);
            if let Some(target) = &after_install_focus {
                target.focus();
            }
            if let Some(onmounted) = onmounted {
                onmounted.call(event);
            }
        }),
        dioxus::html::onkeydown(move |event: KeyboardEvent| {
            if event.key() == Key::Escape {
                on_escape.call(event);
            } else if let Some(onkeydown) = onkeydown {
                onkeydown.call(event);
            }
        }),
    ];
    root_attributes.extend(attributes);
    // GlobalAttributes forwards presentation attributes, but native events
    // need explicit props. Absent listeners remain absent.
    if let Some(onscroll) = onscroll {
        root_attributes.push(dioxus::html::onscroll(move |event: ScrollEvent| {
            onscroll.call(event);
        }));
    }
    if let Some(onresize) = onresize {
        root_attributes.push(dioxus::html::onresize(move |event: ResizeEvent| {
            onresize.call(event);
        }));
    }
    if let Some(onsubmit) = onsubmit {
        root_attributes.push(dioxus::html::onsubmit(move |event: FormEvent| {
            onsubmit.call(event);
        }));
    }
    if let Some(onclick) = onclick {
        root_attributes.push(dioxus::html::onclick(move |event: MouseEvent| {
            onclick.call(event);
        }));
    }
    if let Some(onfocusin) = onfocusin {
        root_attributes.push(dioxus::html::onfocusin(move |event: FocusEvent| {
            onfocusin.call(event);
        }));
    }
    if let Some(onfocusout) = onfocusout {
        root_attributes.push(dioxus::html::onfocusout(move |event: FocusEvent| {
            onfocusout.call(event);
        }));
    }
    match root {
        DialogRoot::Div => rsx! { div { ..root_attributes, {children} } },
        DialogRoot::Form => rsx! { form { ..root_attributes, {children} } },
    }
}
