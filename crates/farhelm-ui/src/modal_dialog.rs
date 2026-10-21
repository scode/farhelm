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

    /// Install once on this root, preserving focus before page isolation.
    ///
    /// The trap uses native focus scrolling on a boundary wrap: a contained
    /// control outside the dialog's scroll viewport must become visible too.
    /// Gap/Enter listeners precede isolation, as in Restart with's installer.
    fn install(self, focus: DialogFocus) {
        let dialog = serde_json::to_string(self.selector).expect("a string always serializes");
        let initial =
            serde_json::to_string(focus.initial_focus).expect("a string always serializes");
        let selector =
            serde_json::to_string(focus.trap.selector).expect("a string always serializes");
        let empty_to_root = focus.trap.empty_to_root;
        let reverse_from_root = focus.trap.reverse_from_root;
        let trap = format!(
            r#"(() => {{
                const dialog = document.querySelector({dialog});
                if (!dialog || dialog.__farhelmDialogTrap) return;
                dialog.__farhelmDialogTrap = true;
                dialog.querySelector({initial})?.focus({{ preventScroll: true }});
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
        );
        let gap = crate::dialog_focus::install_js(
            self.selector,
            crate::dialog_focus::ContainerEnter::Click(focus.main_action),
        );
        let isolate = crate::modal_isolation::install_js(self.selector, Some(focus.escape_target));
        document::eval(&format!("{trap}\n{gap}\n{isolate}"));
    }
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

/// Declare focus targets independently of the caller's cancellation rules.
///
/// Initial focus need not be the primary action. Escape's safety-net target
/// remains separate too: its ordinary button handler and native disabled state
/// decide whether an outside-focus Escape may cancel a busy operation.
#[derive(Clone, Copy, PartialEq)]
pub(crate) struct DialogFocus {
    pub(crate) initial_focus: &'static str,
    pub(crate) trap: TabTrap,
    pub(crate) escape_target: &'static str,
    pub(crate) main_action: &'static str,
}

/// Render one existing modal root and own its mount-time focus boundary.
///
/// Attributes remain caller-supplied so markup, labels and root focusability
/// stay unchanged. Escape delegates to the caller's live state and layered
/// cancellation rules; other keys keep their existing root handler. Child
/// focus handoffs remain independent, and ordinary rerenders never repeat
/// initial focus over a later child question.
#[component]
pub(crate) fn ModalDialog(
    handle: DialogHandle,
    focus: DialogFocus,
    on_escape: EventHandler<KeyboardEvent>,
    onkeydown: EventHandler<KeyboardEvent>,
    #[props(extends = GlobalAttributes)] attributes: Vec<Attribute>,
    children: Element,
) -> Element {
    rsx! {
        div {
            onmounted: move |_| handle.install(focus),
            onkeydown: move |event: KeyboardEvent| {
                if event.key() == Key::Escape {
                    on_escape.call(event);
                } else {
                    onkeydown.call(event);
                }
            },
            ..attributes,
            {children}
        }
    }
}
