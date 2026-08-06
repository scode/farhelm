//! The rename control both surfaces share (PLAN_M5.md item 6).
//!
//! SPEC.md puts rename on the session list AND in the session view, and
//! the two want the same widget: a single-line field seeded with the
//! current title, a submit, and a cancel. `RenameForm` is that widget and
//! nothing else — it edits a draft its CALLER owns and hands the finished
//! string back, leaving the request, the optimistic paint, and the error
//! surface to whichever surface mounted it. Keeping it renderer-side-only
//! is what lets `list::SessionRow` and `session_view::SessionView` differ
//! in everything that is genuinely different (where the control sits, what
//! the reply updates, where a refusal is shown) without either of them
//! reimplementing the field.
//!
//! What is deliberately NOT here: any notion of a valid title. The draft
//! is sent verbatim, and every rule about what a title may contain lives
//! in the supervisor (see `api::rename_session`).

use dioxus::prelude::*;

/// One session's rename field: edits `draft`, submits it verbatim.
///
/// ## Why the draft belongs to the caller
///
/// This component is mounted only while a rename is open, and its host can
/// be re-rendered out from under it by something the user did not do — a
/// transient listing-read failure swaps `ListView`'s rows for an error
/// line, which unmounts every row and this form with them. A draft owned
/// here would vanish with it, silently discarding what the user had typed.
/// Owned by the caller it survives that remount, and seeding stays the
/// caller's job too: the draft is set from the CURRENT title at the moment
/// the field is opened, which is also what keeps a read carrying another
/// client's rename from overwriting an edit in progress.
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
/// `busy` disables the whole form for the round trip, which is both the
/// re-entry guard and the honest signal that the request is out. Cancel is
/// refused while busy at the HANDLER too, not only by the attribute: the
/// attribute's rerender is not synchronous with a click, and a cancel that
/// slipped through would unmount the form — throwing away the draft a
/// refusal is about to be reported for.
#[component]
pub(crate) fn RenameForm(
    mut draft: Signal<String>,
    busy: bool,
    on_submit: EventHandler<String>,
    on_cancel: EventHandler<()>,
) -> Element {
    rsx! {
        form {
            class: "rename-form",
            onsubmit: move |evt| {
                evt.prevent_default();
                // Signal-level re-entry check, not just the `disabled`
                // attributes below: a rerender disabling them is not
                // synchronous with the event that queued this submit.
                if busy {
                    return;
                }
                on_submit.call(draft());
            },
            textarea {
                class: "rename-input",
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
                        if busy {
                            return;
                        }
                        on_submit.call(draft());
                    }
                },
                autocomplete: "off",
                autocorrect: "off",
                autocapitalize: "none",
                spellcheck: "false",
                // Focus lands in the field the instant it appears, so
                // renaming is type-then-Enter. The plain HTML attribute
                // rather than a fallible `set_focus` whose discarded
                // `Result` could silently drop the behavior — the same
                // choice the confirm prompts make for their cancel button.
                autofocus: true,
                value: "{draft}",
                disabled: busy,
                oninput: move |evt| draft.set(evt.value()),
            }
            button {
                r#type: "submit",
                class: "btn rename-submit",
                disabled: busy,
                "save"
            }
            button {
                r#type: "button",
                class: "btn rename-cancel",
                disabled: busy,
                onclick: move |_| {
                    if busy {
                        return;
                    }
                    on_cancel.call(());
                },
                "cancel"
            }
        }
    }
}
