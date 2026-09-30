# Dropping a file on a catching-up or reconnecting terminal can navigate the page away

Reviewed commit: de774a1ee8815ce833da77deac593a55d82f7be3

## TLDR

In the browser, dropping a file onto a terminal that is still loading its history, or showing "reconnecting", can
replace the Farhelm page with the dropped file. Every terminal on the page goes down with it. The terminal code already
says this must never happen.

## Details

Source: gap-filling review pass, 2026-09-30, slice ui-terminal.

Reviewer's confidence: likely (unverified premise: the browser's default for a file drop nobody claims is to open the
file in the tab; that is standard Chromium/Safari behavior, not reproduced here).

Reviewer's bucket suggestion: other.

Possible cover for triage to check: none

- The drop guard (`preventDefault` on `dragover` and `drop`) is installed only on the island's own mount element
  (`installAttachments`, `terminal.js` lines 2290-2297). Its comment says an unhandled drop would navigate to the file
  and take the page down.
- That element is `visibility: hidden` for:
  - every catch-up (line 3001);
  - every reconnect attempt: an attempt that has not proved its attach is never revealed (`reveal`, line 3829), and
    failed attempts stay mounted hidden until the next one, so this covers essentially the whole recovery after the
    first rung.
- Hidden elements are not hit-test targets. The overlays above them (`.terminal-connecting`, `.attach-status`) are
  `pointer-events: none` (`app.css` about lines 4930 and 4975). So the drop lands on the pane or its ancestors.
- Nothing there cancels `dragover`. There is no document-level guard anywhere in `src/` or `assets/`, and `desktop.rs`
  notes that no `ondrop` handler exists.
- The same holds for panes with no island at all: a stale or interrupted session (empty desired set), or tabs beyond
  `MAX_MOUNTED_TAB_ISLANDS`.

On the web this lets the engine's default drop action run, and the documented "not connected, nothing attached" refusal
never appears. The desktop build probably refuses the file:// navigation through Dioxus's navigation handler; that is
not verified.

Fix: a document-level (or `.terminal-panes`-level) `dragover`/`drop` listener that always prevents the default.
Optionally route drops on a pane with no live island to the "not connected" message.
