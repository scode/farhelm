# Rename editor conceals title characters — Editable title seed

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

The rename editor hides meaningful characters in its editable title.

## Details

F222 — **definite** — `crates/farhelm-ui/src/rename.rs:166` — Rename editor conceals title characters — Editable title
seed

The dialog seeds its textarea directly from the raw session title. Valid invisible or directional format characters can
remain concealed while Save preserves them, making inspection and editing misleading. Use an escaped initial display
with separate raw-byte ownership and explicit edit tracking, preserving untouched raw bytes rather than saving the
escaped display. This finding concerns the editable seed, not the separate original-title display.

## Evidence and triage context

- crates/farhelm-ui/src/list/row.rs:983: rename_start contains session.title.clone().
- crates/farhelm-ui/src/list/view.rs:2486-2499: the raw title becomes rename_draft and the editor's current_title.
- crates/farhelm-ui/src/list/view.rs:3807-3815: those values are passed directly to RenameDialog.
- crates/farhelm-ui/src/rename.rs:166 and 260: the textarea value and original-title span interpolate the supplied
  strings without display_peer.
- crates/farhelm-supervisor/src/service/core.rs:3448-3455: title validation rejects char::is_control, not format
  characters such as U+202E or U+200B.
- crates/farhelm-ui/src/list/row.rs:583-596: the sidebar's PeerTitle instead calls display_peer and uses direction
  isolation.
- crates/farhelm-ui/src/peer.rs:63-84: display_peer makes presentation-unsafe characters visible.
- crates/farhelm-ui/assets/app.css:2799-2829: the editor's styling supplies no equivalent escaping.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- {"basis": "TRIAGE_OUTCOMES.md:2314-2331, titles-raw-in-confirm-prompts.md", "comparison": "Same input class, but the
  recorded consequence and repair concern sidebar titles and Delete/Replace confirmations. Rename's original-title
  display and editable seed are independently editable sites."}
- {"basis": "TRIAGE_OUTCOMES.md:2290-2312, session-header-raw-peer-text.md", "comparison": "Covers header displays and
  raw clipboard copying, not the rename editor."}
- {"basis": "SPEC_impl.md:598-603", "comparison": "Specifies escaped-display/raw-seed/edited-flag handling for cloned
  fields. This supplies an applicable implementation pattern, not acceptance of raw rename rendering."}

Caveats:

- The confirmed consequence is misleading display and editing, not script execution or a demonstrated wrong-session
  operation.
- Dioxus text interpolation prevents HTML interpretation.
- Preserve untouched stored bytes and deliberate edits; simply submitting display_peer output would introduce a separate
  mutation bug.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `ui_desktop_14_sec:p1:F2`.

- `ui_desktop_14_sec:p1:F2`: confidence as filed: definite; suggested bucket as filed: other.
