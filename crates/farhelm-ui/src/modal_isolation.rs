//! Make a mounted modal the only part of the page that can hold focus.
//!
//! A modal in this app is ordinary DOM rendered inside a live page: the
//! session view's terminal, the sidebar and the header all stay mounted under
//! its scrim. `aria-modal` describes that relationship to assistive
//! technology but constrains nothing, and a per-control focus trap only sees
//! keys that already arrive inside the dialog. Focus that leaves by some other
//! route (another component's `focus()` call, a click on the scrim, a focused
//! control that is disabled or unmounted) is then free to walk into whatever
//! is behind it. Behind the restart-with dialog that is an agent's terminal,
//! where a stray keystroke becomes agent input.
//!
//! Isolation closes those routes structurally, in two layers:
//!
//! - **`inert` on everything outside the dialog.** Every sibling of every
//!   node on the path from the dialog up to `body` gets the attribute. An
//!   inert subtree cannot be focused (`element.focus()` is a no-op), is
//!   skipped by Tab, ignores the pointer, and leaves the accessibility tree,
//!   so the question "which code might still call `focus()` behind the
//!   modal" stops mattering. The dialog renders deep inside the session view,
//!   which is why this walks the ancestor chain rather than marking one app
//!   root. Siblings added to a path node while the dialog is mounted (a band
//!   the view renders later, a container a reattachment re-creates) are
//!   marked too.
//! - **A capture-phase `keydown` safety net.** If focus is nevertheless
//!   outside the dialog when a key arrives (on `body` after a scrim click, or
//!   in an engine without `inert`), the key is swallowed and focus goes back
//!   to the dialog container. Losing one keystroke is the intended trade: the
//!   alternative is delivering it to whatever is behind the modal. Escape is
//!   the exception that still does its job, by clicking the dialog's own
//!   cancel control (so a cancel that is disabled while busy does nothing).
//!
//! Ownership is exact. Only elements that did NOT already carry `inert` are
//! marked and recorded, and release removes the attribute from exactly those,
//! so a page that inerts something for its own reasons keeps it.
//!
//! Release is idempotent and has two triggers. The dialog's close path calls
//! it synchronously (see [`release_js`]) before it schedules focus back to
//! the element that opened the dialog, because that element is inert until
//! release and `focus()` on it would silently fail. A `MutationObserver` on
//! the path nodes also releases once the dialog leaves the document, which
//! covers every unmount the close path does not see (the whole view
//! unmounting, say); without it the page would stay frozen.
//!
//! Engines that do not implement `inert` ignore the attribute. They keep the
//! safety net and whatever per-dialog layers the caller has.
//!
//! Only the restart-with dialog uses this today. The rename dialog and the
//! session launcher have the same shape and could adopt it.

/// JavaScript that isolates the dialog matched by `dialog_selector`.
///
/// Run it after the dialog's initial focus has landed inside it: inerting an
/// ancestor of the focused element blurs it, and the dialog's own focus
/// handoff must win over whatever held focus before the dialog opened.
/// Installing twice on the same element is a no-op, so an `onmounted` that
/// fires again cannot stack listeners.
///
/// `escape_selector` names the control, inside the dialog, that Escape
/// clicks when the safety net catches it with focus outside the dialog. It
/// should be the dialog's cancel action, natively disabled whenever Escape
/// must not close (`click()` on a disabled control does nothing). `None`
/// makes the net swallow Escape like any other key.
pub(crate) fn install_js(dialog_selector: &str, escape_selector: Option<&str>) -> String {
    ISOLATE_TEMPLATE
        .replace("__DIALOG__", &js_string(dialog_selector))
        .replace(
            "__ESCAPE__",
            &escape_selector.map_or_else(|| "null".to_string(), js_string),
        )
}

/// JavaScript that releases the isolation of the dialog matched by
/// `dialog_selector`, if any, synchronously.
///
/// Prefix a close path's focus return with this: the element that should
/// receive focus back is outside the dialog and so still inert until release
/// runs. Safe to run when nothing is isolated or the dialog is already gone.
pub(crate) fn release_js(dialog_selector: &str) -> String {
    format!(
        "document.querySelector({})?.__farhelmModalRelease?.();",
        js_string(dialog_selector)
    )
}

/// Quote a selector as a JavaScript string literal.
///
/// JSON string syntax is a subset of JavaScript's, so this is exact for any
/// selector, including ones with quotes (`[role="dialog"]`) in them.
fn js_string(value: &str) -> String {
    serde_json::to_string(value).expect("a string always serializes")
}

/// The isolation installer; `__DIALOG__` and `__ESCAPE__` are JS literals.
const ISOLATE_TEMPLATE: &str = r#"(() => {
    const dialog = document.querySelector(__DIALOG__);
    if (!dialog || dialog.__farhelmModalIsolated) return;
    dialog.__farhelmModalIsolated = true;
    const escapeSelector = __ESCAPE__;

    // The dialog and its ancestors up to body stay live; every other element
    // hanging off that chain is outside the modal.
    const path = [];
    for (let node = dialog; node && node !== document.documentElement; node = node.parentElement) {
        path.push(node);
    }
    const onPath = new Set(path);
    const owned = new Set();
    const claim = (node) => {
        if (!(node instanceof Element) || onPath.has(node) || node.hasAttribute('inert')) return;
        node.setAttribute('inert', '');
        owned.add(node);
    };
    const parents = path.slice(1);
    for (const parent of parents) {
        for (const child of parent.children) claim(child);
    }

    const onKeydown = (event) => {
        if (!dialog.isConnected) {
            release();
            return;
        }
        const active = document.activeElement;
        if (active && dialog.contains(active)) return;
        event.preventDefault();
        event.stopPropagation();
        dialog.focus({ preventScroll: true });
        if (event.key === 'Escape' && !event.isComposing && escapeSelector) {
            dialog.querySelector(escapeSelector)?.click();
        }
    };

    // childList without subtree: only the path nodes' own children matter,
    // and a subtree observer would wake for every line of terminal output.
    const observer = new MutationObserver((records) => {
        if (!dialog.isConnected) {
            release();
            return;
        }
        for (const record of records) {
            for (const node of record.addedNodes) claim(node);
        }
    });

    function release() {
        if (dialog.__farhelmModalRelease !== release) return;
        dialog.__farhelmModalRelease = null;
        observer.disconnect();
        document.removeEventListener('keydown', onKeydown, true);
        for (const node of owned) node.removeAttribute('inert');
        owned.clear();
    }

    dialog.__farhelmModalRelease = release;
    for (const parent of parents) observer.observe(parent, { childList: true });
    document.addEventListener('keydown', onKeydown, true);
})();"#;

#[cfg(test)]
mod tests {
    use super::*;

    /// Selectors travel into JavaScript as string literals.
    ///
    /// The restart-with selector carries double quotes (`[role="dialog"]`);
    /// naive interpolation would end the literal early and the installer
    /// would throw before isolating anything, silently leaving the page live.
    #[test]
    fn selectors_are_embedded_as_exact_string_literals() {
        let js = install_js(r#".x-dialog[role="dialog"]"#, Some(".x-cancel"));
        assert!(js.contains(r#"document.querySelector(".x-dialog[role=\"dialog\"]")"#));
        assert!(js.contains(r#"const escapeSelector = ".x-cancel";"#));
        assert!(!js.contains("__DIALOG__") && !js.contains("__ESCAPE__"));

        let js = install_js(".x-dialog", None);
        assert!(js.contains("const escapeSelector = null;"));

        assert_eq!(
            release_js(r#".x-dialog[role="dialog"]"#),
            r#"document.querySelector(".x-dialog[role=\"dialog\"]")?.__farhelmModalRelease?.();"#
        );
    }
}
