//! The rename editor's draft and keyboard semantics.
//!
//! `RenameForm` owns only the native textarea behavior: a caller supplies
//! the draft and receives a verbatim submission or cancellation. `RenameDialog`
//! gives the list that field a stable modal parent, leaving requests,
//! optimistic paint, and response ownership in `ListView`.
//!
//! What is deliberately NOT here: any notion of a valid title. The draft
//! is sent verbatim, and every rule about what a title may contain lives
//! in the supervisor (see `api::rename_session`).

use dioxus::prelude::*;

/// Keep Tab inside the one-purpose rename dialog while it is mounted.
///
/// The sidebar stays live behind the modal. Native modal attributes describe
/// that relationship to assistive technology but do not constrain browser
/// focus in every renderer, so the listener belongs to this exact dialog
/// node and vanishes with it.
fn install_rename_focus_trap(generation: u64) {
    document::eval(
        &r#"(() => {
            const dialog = document.querySelector('.rename-dialog[role="dialog"]');
            if (!dialog || dialog.dataset.renameGeneration !== '__GENERATION__' || dialog.__farhelmRenameFocusTrap) return;
            dialog.__farhelmRenameFocusTrap = true;
            // Autofocus may already have been consumed by an earlier editor
            // in this document. Mount owns one focus handoff, but a delayed
            // bridge callback must yield to an outside control chosen since.
            const active = document.activeElement;
            if (!active || active === document.body || active.matches('.session-row-rename')) {
                dialog.querySelector('.rename-input')?.focus({ preventScroll: true });
            }
            const focusable = () => [...dialog.querySelectorAll(
                'button:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])',
            )].filter((node) => !node.hidden && node.getClientRects().length);
            dialog.addEventListener('keydown', (event) => {
                if (event.key !== 'Tab') return;
                const nodes = focusable();
                if (!nodes.length) return;
                const first = nodes[0];
                const last = nodes[nodes.length - 1];
                if (event.shiftKey ? document.activeElement === first : document.activeElement === last) {
                    event.preventDefault();
                    (event.shiftKey ? last : first).focus();
                }
            });
        })();"#.replace("__GENERATION__", &generation.to_string()),
    );
}

/// One session's rename field: edits `draft`, submits it verbatim.
///
/// ## Why the draft belongs to the caller
///
/// The dialog can be replaced only by a deliberate close, but its parent
/// still re-renders for listing updates the user did not cause. A draft owned
/// by the field would be lost with any future surface change; caller ownership
/// keeps cancellation, a refusal, and authoritative source removal from
/// discarding it. Seeding remains the caller's job too: the draft is set from
/// the current title when the editor opens, so another client's listing update
/// cannot overwrite an edit in progress.
///
/// ## A textarea, not a text input, and why that is not cosmetic
///
/// `<input type="text">` applies the HTML value-sanitization algorithm,
/// which STRIPS carriage returns and line feeds from anything put in it —
/// so a pasted multi-line title would reach the supervisor silently
/// altered, which is exactly the rewrite-the-caller's-data move this
/// feature refuses to make, and it would also mean the client quietly
/// repaired a title the supervisor exists to REFUSE (a line feed is a
/// control character). A `<textarea>` preserves what was pasted, so the
/// refusal comes from the authority that owns the rule.
///
/// It is made to BEHAVE like a single-line field rather than look like a
/// text area: one row, no resize handle, no wrapping (app.css), and Enter
/// submits instead of inserting a newline. Typing therefore cannot
/// manufacture the multi-line case at all — only a paste can, which is the
/// case that has to reach the server intact.
///
/// ## Verbatim, and therefore no client-side validation
///
/// `on_submit` receives the field's contents exactly — no trim, no
/// emptiness check, no control-character screening. An empty title is a
/// legal rename (create accepts an explicit empty title, and rename
/// inventing a stricter rule would be an asymmetry SPEC.md nowhere asks
/// for), and a title the supervisor refuses comes back as its own words
/// for the caller to show. Duplicating those rules here would give them a
/// second place to drift from.
///
/// The field opts out of every browser text "correction" for the reason
/// `list::CreateSessionForm`'s title field does: whatever the user types
/// is what must come back out (observed directly — WKWebView's autocorrect
/// silently capitalizing a typed word in place), and a rename that quietly
/// altered the caller's own data would defeat the same contract.
///
/// `busy` makes the field read-only for the round trip. It deliberately does
/// not disable it: the user may need to copy the exact submitted title, and
/// keeping the focused field in the tab order prevents the browser from
/// dropping focus onto the document body. Save follows the same rule with
/// `aria-disabled`; a click-submitted request keeps focus on the action the
/// user chose. The event handlers remain the authority because a rerender is
/// not synchronous with the event that started the request.
#[component]
pub(crate) fn RenameForm(
    mut draft: Signal<String>,
    busy: bool,
    /// Reads the parent operation set at event time. Rendered attributes can
    /// lag the event that starts a request, so they are not an authority.
    busy_now: Callback<(), bool>,
    /// Blocks submission while leaving the draft selectable and Cancel usable.
    /// A complete listing can prove the source vanished; that is not a reason
    /// to lock the text a user may need to copy before dismissing the dialog.
    submit_disabled: bool,
    on_submit: EventHandler<String>,
    on_cancel: EventHandler<()>,
) -> Element {
    rsx! {
        form {
            class: "rename-form",
            onsubmit: move |evt| {
                evt.prevent_default();
                // Parent-state re-entry check, not just the rendered
                // attributes below: their rerender is not synchronous with
                // the event that queued this submit.
                if busy_now.call(()) || submit_disabled {
                    return;
                }
                on_submit.call(draft());
            },
            textarea {
                class: "rename-input",
                aria_label: "new session title",
                rows: "1",
                // A textarea does not submit its form on Enter, so the
                // single-line behavior is implemented here: Enter submits
                // and never inserts a line break, which is what leaves the
                // multi-line case reachable only by pasting — see this
                // component's docs for why that case must survive.
                //
                // Except mid-COMPOSITION, where the same key means
                // "accept the candidate I am looking at" to every IME
                // there is. Submitting there would send a half-composed
                // title — and, worse, would do it on the keystroke a
                // Japanese or Chinese typist uses to finish nearly every
                // word, making the field unusable rather than merely
                // surprising. The composing keydown is left entirely
                // alone (no `prevent_default` either), so the IME's own
                // handling of it is untouched.
                onkeydown: move |evt| {
                    if evt.key() == Key::Enter && !evt.is_composing() {
                        evt.prevent_default();
                        if busy_now.call(()) || submit_disabled {
                            return;
                        }
                        on_submit.call(draft());
                    }
                },
                autocomplete: "off",
                autocorrect: "off",
                autocapitalize: "none",
                spellcheck: "false",
                // Native autofocus handles first insertion. The dialog's
                // mount handoff also covers later editors in a document
                // where the browser has already consumed autofocus.
                autofocus: true,
                value: "{draft}",
                readonly: busy,
                oninput: move |evt| {
                    // A renderer can deliver an input event queued just
                    // before `readonly` reached the DOM. The submitted
                    // snapshot must remain the visible draft until the
                    // request answers, so the handler carries the same gate.
                    if !busy_now.call(()) {
                        draft.set(evt.value());
                    }
                },
            }
            button {
                r#type: "submit",
                class: "btn btn-primary rename-submit",
                disabled: submit_disabled,
                aria_disabled: if busy { "true" },
                onclick: move |evt| {
                    if busy_now.call(()) {
                        evt.prevent_default();
                    }
                },
                "save"
            }
            button {
                r#type: "button",
                class: "btn rename-cancel",
                disabled: busy,
                onclick: move |_| {
                    if busy_now.call(()) {
                        return;
                    }
                    on_cancel.call(());
                },
                "cancel"
            }
        }
    }
}

/// The stable modal home for a list-owned rename draft.
///
/// A row menu is anchored to geometry that can disappear or move whenever a
/// listing changes. This dialog deliberately is not: its parent survives
/// listing errors, filters, and keyed row reordering, so the textarea keeps
/// its DOM identity, selection, and IME composition for the whole editing
/// lifetime. `generation` identifies that lifetime to callers; a session id
/// alone is insufficient for deferred work because a later editor for that
/// same session is a different focus and draft owner. The product UI refuses
/// reopening while a request is pending, but the result boundary does not
/// rely on that reachability rule for safety.
#[component]
pub(crate) fn RenameDialog(
    draft: Signal<String>,
    busy: bool,
    busy_now: Callback<(), bool>,
    unavailable: bool,
    current_title: String,
    error: Option<String>,
    generation: u64,
    focus_owned: Signal<bool>,
    on_submit: EventHandler<(u64, String)>,
    on_cancel: EventHandler<u64>,
) -> Element {
    rsx! {
        div {
            class: "rename-dialog-backdrop",
            role: "presentation",
        div {
            class: "rename-dialog",
            "data-rename-generation": "{generation}",
            role: "dialog",
            aria_modal: "true",
            aria_label: "rename session",
            onmounted: move |_| install_rename_focus_trap(generation),
            // Focusout followed by focusin is an internal transition and
            // ends owned. A move outside has no matching focusin and ends
            // unowned. Busy controls stay focusable, so the render itself
            // cannot manufacture the external case.
            onfocusin: move |_| focus_owned.set(true),
            onfocusout: move |_| focus_owned.set(false),
            onkeydown: move |evt| {
                // Escape accepts or dismisses candidates while an IME is
                // composing. Treating that key as dialog cancellation would
                // abandon the draft on an ordinary composition keystroke.
                if evt.key() == Key::Escape
                    && !evt.is_composing()
                    && !busy_now.call(())
                {
                    on_cancel.call(generation);
                }
            },
            span { class: "rename-current-title", "{current_title}" }
            if unavailable {
                p {
                    class: "rename-unavailable",
                    "this session is no longer available; copy the draft or cancel"
                }
            }
            if let Some(error) = error {
                p { class: "rename-error", "{error}" }
            }
            RenameForm {
                draft,
                busy,
                busy_now,
                submit_disabled: unavailable,
                on_submit: move |title| on_submit.call((generation, title)),
                on_cancel: move |_| on_cancel.call(generation),
            }
        }
        }
    }
}
