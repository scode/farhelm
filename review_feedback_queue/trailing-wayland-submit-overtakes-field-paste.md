# Wayland: a submit right after a paste sends the field without the pasted text

Reviewed commit: e4e6f22246480d8f9b6cdcd5a29d209465745499

## TLDR

On Wayland, pasting into a text field and pressing Enter (or clicking the form's submit button) before the other program
has handed over the clipboard text submits what the field held before the paste. In the SSH sign-in prompt that sends an
empty or partial password to the host, spending one of the server's authentication attempts, and the user then sees an
empty field and a failed sign-in. A launcher or rename field acts on the old text. Before this commit the paste blocked
the window until the text arrived, so a quick Ctrl+V Enter always submitted the pasted text.

## Details

Provenance: trailing review through `dbb153ec`, group g01 (Wayland asynchronous text-field paste), finding f1 (revised
in synthesis). Suggested bucket: `other`. Confidence: likely.

- `Clipboard::read` (`vendor/gpui-pre-linux/src/linux/wayland/clipboard.rs:693-699` at the tip) now returns `None` for
  another program's selection, so GPUI Kit's text-input paste (gpui-base 0.9.1, `src/input/base/state.rs:3217-3250`,
  unpatched registry source) always takes its async branch. It spawns a task awaiting `read_from_clipboard_async` and
  inserts only if the field is still editable, focused and its paste target (document revision and selections) is
  unchanged.
- Enter in a single-line field emits `Submitted` without editing the document, so it neither waits for nor cancels the
  pending paste. Key events queued behind Ctrl+V are dispatched before the pipe answer is applied, because the read
  completes on a background thread (`receive_payload`, `clipboard.rs:468-500`) and is applied by a later foreground
  wakeup.
- Confirmed consumer: `crates/gui/farhelm-gui/src/sign_in_form.rs:42-48` subscribes to `TextInputEvent::Submitted`;
  `submit` reads `field.text(cx)` as it is at that moment (`:102`) and POSTs `{action: send, answer}` (`:111`) with no
  empty or pending-paste check. The field is made read-only while sending, so the late paste is then dropped by Kit's
  editability check.
- The SPEC.md change in this commit (`SPEC.md:3758-3760`, "Terminal experience" clipboard bullet) lists the ways a
  pending field paste is dropped (typing, cursor move, leaving, pasting again) but not a submit overtaking it.

Fix options: have Farhelm's text input wrapper (`crates/gui/farhelm-gui/src/components/text_input.rs`) track a pending
paste and hold `Submitted` until it resolves (bounded by the pipe timeout), or drop the pending paste on submit and add
that case to SPEC.md beside the others.

Unverified premise: that with a normally responsive clipboard owner, Enter typed right after Ctrl+V is in practice
handled before the answer is applied. Not observed; with a slow owner, the case this commit targets, it clearly is.
Would move to `high` if shown common (for example a password manager or a Ctrl+V Enter habit regularly racing a normal
clipboard owner).

Possible cover: none known. The SPEC.md clipboard bullet quoted above is adjacent but does not name submit.
