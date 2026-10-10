# Plain clicks can overwrite the clipboard with an old selection

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

An ordinary terminal click can replace the clipboard with an old selection.

## Details

F26 — **definite** — `crates/farhelm-ui/assets/copy-on-select.js:76` — Plain clicks can overwrite the clipboard with an
old selection

Under legacy mouse reporting, a forced local selection can remain after a subsequent ordinary application click.
Farhelm's copy-on-select handler checks the retained selection but does not check whether that click could select text
locally. If the user copied something elsewhere in between, the click overwrites that newer clipboard content with the
old terminal selection. Require an eligible local-selection gesture before copying, while preserving deliberate
reselection of identical text.

## Evidence and triage context

- crates/farhelm-ui/assets/vendor/xterm.js:1 contains SelectionService.handleMouseDown, which returns without clearing a
  retained selection when selection is disabled and the press does not force selection.
- crates/farhelm-ui/assets/vendor/xterm.js:1 routes DEFAULT mouse encoding through triggerBinaryEvent; that method does
  not fire onUserInput, which is the selection-clearing input event.
- crates/farhelm-ui/assets/terminal.js:5237 records every press, but line 5295 passes only the retained selection to the
  copy predicate and line 5302 enqueues it.
- crates/farhelm-ui/assets/copy-on-select.js:76 accepts any nonempty retained selection.

Retained confidence: **definite**. Suggested bucket: **highest**.

Possible cover:

- SPEC.md:1253 describes completed local selections. Its OSC 52 allowance authorizes a different write path and does not
  cover an accidental stale-selection copy. No matching existing item found.

Caveats:

- No interactive reproduction was performed.
- The confirmed path requires legacy/default mouse encoding and a retained forced selection.
- No clipboard exfiltration is alleged.
- Requires default/legacy mouse encoding and a retained forced selection.
- No interactive reproduction.
- The input's xterm line-5 references are inaccurate in this checkout: the relevant minified methods are on physical
  line 1.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `ui_desktop_02_cor:p1:F1`.

- `ui_desktop_02_cor:p1:F1`: confidence as filed: definite; suggested bucket as filed: highest.
