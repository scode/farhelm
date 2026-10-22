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
/// on a child hook remaining mounted. The selector must identify the intended
/// instance; this handle does not arbitrate multiple dialogs or queue them.
#[derive(Clone, Copy, PartialEq)]
pub(crate) struct DialogHandle {
    selector: &'static str,
}

impl DialogHandle {
    /// Bind focus operations to the root whose lifecycle the caller owns.
    pub(crate) const fn new(selector: &'static str) -> Self {
        Self { selector }
    }

    /// Release isolation now and return focus after the closing render.
    ///
    /// The supplied body chooses the destination for this close reason. It
    /// contains only that return action: release and frame scheduling live
    /// here so no caller can focus its still-inert opener first. A missing
    /// destination retains the caller's existing no-op behavior.
    pub(crate) fn close(self, return_focus_body: &str) {
        document::eval(&format!(
            "{} requestAnimationFrame(() => {{ {return_focus_body} }});",
            crate::modal_isolation::release_js(self.selector),
        ));
    }

    /// Release for a modal handoff without scheduling a competing focus return.
    ///
    /// The next modal owns its initial focus. Ordinary DOM-removal cleanup is
    /// still the fallback for success paths that do not explicitly close.
    pub(crate) fn release(self) {
        document::eval(&crate::modal_isolation::release_js(self.selector));
    }

    /// Install the opening's focus policy without repeating it on rerenders.
    fn install(self, focus: &DialogFocus) {
        install_focus(self.selector, focus);
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
fn trap_js(dialog: &str, trap: TabTrap, initial_focus: &str) -> String {
    let selector = serde_json::to_string(trap.selector).expect("a string always serializes");
    let empty_to_root = trap.empty_to_root;
    let reverse_from_root = trap.reverse_from_root;
    format!(
        r#"(() => {{
                const dialog = document.querySelector({dialog});
                if (!dialog || dialog.__farhelmDialogTrap) return;
                dialog.__farhelmDialogTrap = true;
                {initial_focus}
                const focusable = () => [...dialog.querySelectorAll({selector})]
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
                }});
            }})();"#,
    )
}

/// Install the shared boundary from a shell or an as-yet-unconverted host root.
///
/// Initial focus precedes isolation, which otherwise blurs an inerted opener.
/// Remembered dialogs keep their existing two eval submissions: focus and
/// isolation first, gap/Enter second. Direct trapped dialogs keep gap/Enter
/// before isolation in one eval. These are mount contracts, not effects that
/// should run again whenever a dialog's state changes.
pub(crate) fn install_focus(dialog_selector: &str, focus: &DialogFocus) {
    let dialog = serde_json::to_string(dialog_selector).expect("a string always serializes");
    let setup = match &focus.initial_focus {
        InitialFocus::Target(selector) => {
            let target = FocusTarget::new(selector.clone(), None, false);
            let initial = target_js("dialog", &target, false);
            if let Some(trap) = focus.trap {
                trap_js(&dialog, trap, &initial)
            } else {
                format!(
                    "(() => {{ const dialog = document.querySelector({dialog}); if (dialog) {{ {initial} }} }})();"
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
                    if (!dialog) return;
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
                setup.push_str(&trap_js(&dialog, trap, ""));
            }
            setup
        }
    };
    let action = focus.main_action.as_deref().map_or(
        crate::dialog_focus::ContainerEnter::None,
        crate::dialog_focus::ContainerEnter::Click,
    );
    let gap = crate::dialog_focus::install_js(dialog_selector, action);
    let isolate =
        crate::modal_isolation::install_js(dialog_selector, focus.escape_target.as_deref());
    match &focus.initial_focus {
        InitialFocus::Target(_) => {
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
}

/// Keep mount targets separate from live cancellation and request ownership.
///
/// Missing trap and main action retain a dialog's current native Tab and Enter
/// behavior; initial focus never implies that the same action handles Escape.
#[derive(Clone, PartialEq)]
pub(crate) struct DialogFocus {
    pub(crate) initial_focus: InitialFocus,
    pub(crate) trap: Option<TabTrap>,
    pub(crate) escape_target: Option<String>,
    pub(crate) main_action: Option<String>,
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
        }
    }
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
/// An optional second focus target preserves setup's existing root-first
/// isolation, then quiet confirmation handoff, before domain mounted work.
#[component]
pub(crate) fn ModalDialog(
    handle: DialogHandle,
    focus: DialogFocus,
    on_escape: EventHandler<KeyboardEvent>,
    #[props(default)] after_install_focus: Option<FocusTarget>,
    #[props(default)] onkeydown: Option<EventHandler<KeyboardEvent>>,
    #[props(default)] onmounted: Option<EventHandler<MountedEvent>>,
    #[props(default)] onscroll: Option<EventHandler<ScrollEvent>>,
    #[props(default)] onresize: Option<EventHandler<ResizeEvent>>,
    #[props(extends = GlobalAttributes)] attributes: Vec<Attribute>,
    children: Element,
) -> Element {
    // GlobalAttributes forwards presentation attributes, but native movement
    // events need explicit props. Keep absent listeners absent on other roots.
    let mut attributes = attributes;
    if let Some(onscroll) = onscroll {
        attributes.push(dioxus::html::onscroll(move |event: ScrollEvent| {
            onscroll.call(event);
        }));
    }
    if let Some(onresize) = onresize {
        attributes.push(dioxus::html::onresize(move |event: ResizeEvent| {
            onresize.call(event);
        }));
    }
    rsx! {
        div {
            onmounted: move |event| {
                handle.install(&focus);
                if let Some(target) = &after_install_focus {
                    target.focus();
                }
                if let Some(onmounted) = onmounted {
                    onmounted.call(event);
                }
            },
            onkeydown: move |event: KeyboardEvent| {
                if event.key() == Key::Escape {
                    on_escape.call(event);
                } else if let Some(onkeydown) = onkeydown {
                    onkeydown.call(event);
                }
            },
            ..attributes,
            {children}
        }
    }
}
