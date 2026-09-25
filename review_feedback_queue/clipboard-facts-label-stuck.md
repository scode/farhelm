# A permanent clipboard-facts label covers the terminal after any paste

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

After pasting anything into a terminal, a "clipboard facts" label that cannot be clicked stays over the terminal's
bottom-right corner and hides part of the program's display.

## Details

Found by pre-pr-review-swarm run `20260925-0856-2b597e9-339f` (audit of Area 10, UI) as
`F19 / COR-CLIPBOARD-FACTS-STUCK`, tagged **definite**. Anchors and title: `crates/farhelm-ui/assets/terminal.js:2176`,
`crates/farhelm-ui/assets/terminal.js:1732`, `crates/farhelm-ui/assets/terminal.js:1745`,
`crates/farhelm-ui/assets/app.css:4499` — After any paste, a permanent, unclickable "clipboard facts" label covers the
terminal's bottom-right corner

`terminal.js`'s paste handler `onPaste` records a diagnostic snapshot of the clipboard (`clipboardFacts`, produced by
`clipboard-name.js`'s `capture()`, which always returns an object) and calls `render()` before it decides whether the
paste is its business (line 2176). It does this for every paste, plain text included. `render()` appends a collapsible
`<details class="clipboard-facts">` to the pane's `.attach-status` overlay and forces the overlay visible (lines
1732–1745). Nothing ever sets `clipboardFacts` back to `null`; it is only replaced by the next paste, and cleared when
the island is disposed. So after the first paste in a terminal, a "clipboard facts" label stays in the bottom-right
corner for the rest of the mount.

It cannot even do its diagnostic job. `.attach-status` is
`position: absolute; right: 0; bottom: 0; pointer-events:
none` (`app.css:4499`) so that drag-selections pass through to
the terminal. `pointer-events` is inherited, and `.clipboard-facts` does not override it, so the summary cannot be
clicked open. `docs/manual-mac-checklist.md` nevertheless tells testers to expand it, and according to the reviewer the
e2e tests only get at it by forcing `details.open = true` from script. Meanwhile it covers the terminal cells that TUIs
use for their status lines.

The suggested fix is to show the facts only when a paste was actually intercepted or a file projection failed (or put
them behind a debug switch), and to give `.clipboard-facts` `pointer-events: auto`.
