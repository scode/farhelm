# Session accessible names retain raw title formatting

Reviewed commit: 5e1f1cfffe38f41e450ab70d440ca8fabac8be36

## TLDR

Session menus and Delete/Replace dialogs expose raw title formatting in their accessible names, while their visible
titles show escaped characters. Whether this can confuse assistive-technology users about a destructive target has not
been demonstrated.

## Details

`crates/farhelm-ui/src/list/row.rs:212–214,1101–1103` passes raw titles to `clamp_title`, which only truncates through
`menu_panel.rs:1019–1031`. The labels reach `list/row.rs:1845,1999,2150`. Applying `display_peer` before clamping would
align the names with the visible title's spelling.

Suggested bucket: **highest provisionally**, only to preserve independent adjudication of the original
destructive-target confusion concern. No security impact, wrong-session activation, XSS or screen-reader reproduction is
demonstrated. Confidence: **confirmed raw accessible-name path**; assistive-technology consequences remain unverified.
Visible `PeerTitle` is escaped and direction-isolated; the raw label is not the visible title, and session IDs are
unchanged.

Possible cover: `TRIAGE_OUTCOMES.md`, `titles-raw-in-confirm-prompts.md` (`2311–2328`), explicitly excludes
accessibility labels from the earlier visible-title fix. The later `SPEC.md:2837–2841` requires escaped host-originated
GUI text wherever it appears. Its decision provenance (`TRIAGE_OUTCOMES.md:8497–8561`) explicitly supersedes an older
identity-label scope but does not mention revoking the accessible-label exclusion. Whether “GUI shows” includes this
output is a product-intent ambiguity, not something source inspection settles. No current queue, Planned or BUGS item
resolves it.

Proposed drop: the originating reviewer conditionally proposed coverage by the earlier explicit exclusion. Independent
drop-review-5's final verdict is **KEEP** because the later broad rule leaves coverage ambiguous. Retain without
claiming the broad rule definitively superseded the specific decision or that assistive-technology harm is established.

Provenance: persistent Astra/high review, `ui-session-list-1/f1`; independent drop-review-5 final KEEP, item 4.
