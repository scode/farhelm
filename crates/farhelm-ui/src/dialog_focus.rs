//! Keep dialog focus stable when the pointer lands on unused space.
//!
//! The browser may focus a dialog's `tabindex="-1"` container after a click
//! between controls. That leaves the next keystroke aimed at the container,
//! even though the user did not choose a different field. Text and controls
//! keep their native mouse behavior; only an empty patch of a dialog or an
//! inline confirmation suppresses the focus-changing default action.

/// What Enter on a focused dialog container means for that surface.
///
/// Each caller names its visible primary action. Initial focus is deliberately
/// separate: some dialogs open on Cancel even though Enter on their container
/// should confirm. The selector is resolved when Enter is pressed, so a button
/// mounted later or disabled by a pending request retains its ordinary rules.
pub(crate) enum ContainerEnter<'a> {
    None,
    Click(&'a str),
}

/// Install one gap-click and optional container-Enter listener on each match.
///
/// This is safe to call again as a dialog rerenders. It never moves focus or
/// bypasses a button's disabled state, and it leaves text selection alone.
pub(crate) fn install_js(dialog_selector: &str, action: ContainerEnter<'_>) -> String {
    let selector = serde_json::to_string(dialog_selector).expect("a string always serializes");
    let enter = match action {
        ContainerEnter::None => "null".to_string(),
        ContainerEnter::Click(selector) => {
            serde_json::to_string(selector).expect("a string always serializes")
        }
    };
    format!(
        r#"(() => {{
            for (const dialog of document.querySelectorAll({selector})) {{
                if (dialog.__farhelmGapFocusInstalled) continue;
                dialog.__farhelmGapFocusInstalled = true;
                dialog.addEventListener('mousedown', (event) => {{
                    if (event.button !== 0 || !(event.target instanceof Element)) return;
                    const target = event.target;
                    if (target.closest('button, input, textarea, select, a[href], label, summary, [contenteditable], [tabindex]:not([tabindex="-1"]), p, span, dt, dd, li, h1, h2, h3, h4, h5, h6')) return;
                    if ([...target.childNodes].some(node => node.nodeType === Node.TEXT_NODE && /\S/.test(node.textContent))) return;
                    event.preventDefault();
                }});
                const enterAction = {enter};
                if (enterAction) dialog.addEventListener('keydown', (event) => {{
                    if (event.key !== 'Enter' || event.target !== dialog || event.repeat || event.isComposing) return;
                    event.preventDefault();
                    event.stopPropagation();
                    dialog.querySelector(enterAction)?.click();
                }});
            }}
        }})();"#
    )
}
