# Two quick Templates departures can leave a stale question over a different draft

Reviewed commit: 470e9e6ceb01d87cfc776fa7dab8bae04ae5d24e

## TLDR

Two outstanding leave questions can cause the remaining answer to save or discard a different template draft.

## Details

f758e3ace8bf2161954f2a54392353448ab41171 replaces an editor-owned inline question with a standalone centered prompt. A
pending prompt does not count as a write and there is no existing-question check in templates/dialog.rs:529–545.
ask_leave at :562–603 captures the departure, opens with no subject, and Save resolves the current dialog editor
(:591–597, :470–477). The new layer is pushed and invalidates the window at layers.rs:1244–1295. Its veil is created
only during rendering (layers.rs:2368–2415). Until that draw, the pointer invokes the prior frame’s still-active
controls (vendor/gpui-pre/src/window.rs:6214–6219, :6423–6457). The shared click path admits those handlers without a
top-layer gate (components/mod.rs:404–415). A second row or Duplicate activation therefore opens another question while
the same edited draft is still present. The newer question can close without closing the older one because closing takes
only its own layer and descendants (layers.rs:1479–1502). Discard on the newer Duplicate question installs a new unsaved
copy (templates/dialog.rs:579–584, :632–648); Save on the older question still invokes the current editor, so it can
save the copy instead of the original edits for which it opened. The existing veil test waits until the first question
is shown before pressing the old control (templates/leave_tests.rs:249–267). SPEC.md:5500–5505 and SPEC_impl.md:973–988
require answers to stay bound to the target presented even when input arrives before redraw. Neither
7223ac49de6d3d5a67b6aceb1c15b4f75d6646a8 nor later in-range template changes add a layer or editor guard. Keep a single
leave question per editor and reject answers after that editor changes.

Confidence: confirmed by inspection.

Provenance: trailing review d3e8a4e8-dfeb-4080-a7b4-080b9065a167, `templates/l9`.
