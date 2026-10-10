# Rename original-title display conceals title characters — Original-title display

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Rename's original-title comparison hides meaningful title characters.

## Details

F228 — **definite** — `crates/farhelm-ui/src/rename.rs:260` — Rename original-title display conceals title characters —
Original-title display

The passive original-title display receives the raw session title without the escaping and direction isolation used in
other title surfaces. Valid invisible or directional characters can consequently conceal the identity being compared
with the edited title. Escape and isolate this display without changing stored bytes. The editable textarea is a
separate sink with its own correction.

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
- crates/farhelm-ui/src/list/row.rs:983 supplies session.title unchanged with the selected session ID.
- crates/farhelm-ui/src/list/view.rs:2486-2496 stores that raw title in the rename editor; :3815 passes it as
  current_title.
- crates/farhelm-ui/src/rename.rs:260 interpolates current_title directly, without display_peer or a direction-isolated
  peer element.
- crates/farhelm-supervisor/src/service/core.rs:3448-3455 rejects char::is_control but permits Unicode format characters
  such as U+200B and U+202E.
- crates/farhelm-ui/src/peer.rs:63-84 implements visible escaping; :115-126 explains the separate direction-isolation
  requirement. Neither is used at this renderer.
- crates/farhelm-ui/assets/app.css:2821-2828 and :2880-2882 apply layout styling, not character escaping or peer-value
  isolation.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- {"basis": "TRIAGE_OUTCOMES.md:2314-2331, titles-raw-in-confirm-prompts.md", "comparison": "Same input class, but the
  recorded consequence and repair concern sidebar titles and Delete/Replace confirmations. Rename's original-title
  display and editable seed are independently editable sites."}
- {"basis": "TRIAGE_OUTCOMES.md:2290-2312, session-header-raw-peer-text.md", "comparison": "Covers header displays and
  raw clipboard copying, not the rename editor."}
- {"basis": "SPEC_impl.md:598-603", "comparison": "Specifies escaped-display/raw-seed/edited-flag handling for cloned
  fields. This supplies an applicable implementation pattern, not acceptance of raw rename rendering."}
- TRIAGE_OUTCOMES.md:2290-2311, session-header-raw-peer-text.md: completed scope is header title, folder, invocation,
  tooltips, and copy feedback.
- TRIAGE_OUTCOMES.md:2314-2330, titles-raw-in-confirm-prompts.md: completed scope is the sidebar row title and
  Delete/Replace confirmation quotes.
- SPEC.md:2166-2169 requires treating remote supervisor messages and agent-controlled output as untrusted. It does not
  accept this display behavior.

Caveats:

- The confirmed consequence is misleading display and editing, not script execution or a demonstrated wrong-session
  operation.
- Dioxus text interpolation prevents HTML interpretation.
- Preserve untouched stored bytes and deliberate edits; simply submitting display_peer output would introduce a separate
  mutation bug.
- The raw rendering and hidden-character ambiguity are definite; practical security severity remains provisional.
- Dioxus renders text nodes, so this is not HTML injection.
- Rename remains bound to the stored session ID; no operation redirected to another ID was demonstrated.
- The editable draft's verbatim submission contract is distinct from the passive source-title display.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `ui_desktop_14_sec:p1:F2`, `ui_desktop_14_cor:p1:F3`.

- `ui_desktop_14_sec:p1:F2`: confidence as filed: definite; suggested bucket as filed: other.
- `ui_desktop_14_cor:p1:F3`: confidence as filed: definite; suggested bucket as filed: highest, provisionally for
  display spoofing.
