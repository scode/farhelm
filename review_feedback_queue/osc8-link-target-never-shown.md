# OSC 8 link targets are never shown before opening

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

Clicking a link printed in a terminal can open a different website than the address the terminal displayed, with no way
to see the real destination first.

## Details

Found by pre-pr-review-swarm run `20260925-0856-2b597e9-339f` (audit of Area 10, UI) as `F32 / SEC-OSC8-TARGET-HIDDEN`,
tagged **possible**. Anchors and title: `crates/farhelm-ui/assets/terminal.js:2979` — OSC 8 hyperlinks from terminal
output open their hidden target on one click without ever showing it

OSC 8 is a terminal escape sequence that makes a piece of text a hyperlink. The displayed text and the link target are
independent, so a program can print "https://github.com/…" while linking somewhere else. The vendored xterm.js, the
terminal widget, supports OSC 8. By default it asks `confirm()` before opening such a link, and that dialog is the one
place the real target used to appear. Farhelm replaced that with a `linkHandler` that has only `activate` (line 2979),
which calls `farhelmTerminalLinks.openTerminalUrl(uri)`. In a browser that is `window.open`. In the desktop app it is a
page navigation that dioxus-desktop's navigation handler intercepts and passes to `webbrowser::open`, i.e. the user's
real default browser. There are no `hover`/`leave` callbacks and no status bar, so the target is never shown before the
click. The only remaining check is xterm's own filter, which restricts OSC 8 to http(s). Plain-text links detected by
the WebLinks addon are unaffected, since their visible text is their target.

SPEC treats terminal output as untrusted, and the only effect on the viewer's machine SPEC grants it is OSC 52 clipboard
writes. With this, any program in a session can show a trustworthy-looking URL and send the click somewhere else, opened
in the user's real browser with their cookies. SPEC has no contract for how links are displayed, and harm requires a
click plus the user trusting the resulting page, hence "possible". The suggested fix is to add `hover` and `leave`
handlers that show the exact target, with host and scheme emphasised and rendered via `textContent`, at least when it
differs from the underlined text, and optionally to confirm when the label's host and the target's host differ.
