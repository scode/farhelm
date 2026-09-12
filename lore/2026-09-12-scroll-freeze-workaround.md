# Scroll-freeze investigation: what is known, what is not, and the workaround

NOTE: Historical artifact, written 2026-09-12 at the end of the investigation that produced PR #556
(`fix: repaint a scrolled-back terminal while output keeps arriving`). It records what was established, what was
believed and then withdrawn, and the shape of the workaround, so that whoever revisits this later starts from the
evidence rather than from the code comment alone. Not maintained; the code and `SPEC_impl.md` are authoritative for
current behavior.

## The report

TODO.md carried this under Near term: scrolling backward becomes unreliable while an agent is actively emitting
output. Frequently the terminal appears split somewhere around the middle: the upper portion freezes while the lower
portion keeps scrolling upward as new output arrives. Scrolling up does not restore normal behavior; after output
stops, scrolling eventually recovers on its own. No reproduction steps, no established cause.

## What was ruled out before anything was built

Farhelm's own JavaScript does nothing to the viewport during live output. Every binary WebSocket frame goes to one
`term.write()` call in `writeBytes` (`crates/farhelm-ui/assets/terminal.js`); the only `scrollToBottom` is the one
at reveal; there is no wheel listener, no batching, and no alternate-screen handling of Farhelm's own. The DOM
renderer is in use (only the fit and clipboard addons load), scrollback is 12,000 lines, and flow control between
browser and supervisor is byte based (a 4 MiB high-water mark on unwritten bytes), so "keeping up" never measured
paint. That left three candidate mechanisms inside vendored xterm.js 6.0.0 and one Farhelm-owned one:

- partial dirty-row refresh keyed to the application's DECSTBM scroll region;
- the `Viewport` scroll-position memo interacting with `BufferService.scroll` decrementing `ydisp` per line when the
  user is scrolled back and the scrollback is full;
- synchronized output (DEC private mode 2026) deferring refreshes until the flush, which matched "recovers after
  output stops";
- a mid-flood `fit.fit()` from the per-island `ResizeObserver` when a sibling band above the panes changes height.

The constraint set before work started, and kept throughout: any fix lives in Farhelm-owned code. The vendored bundle
is not modified, its version is not changed, and no webgl or canvas renderer is adopted.

## The reproduction

`e2e/tests/terminal-scroll-freeze.spec.ts` drives a real session with a fake agent that keeps emitting, scrolls the
terminal back with real `page.mouse.wheel` events (no `term.scrollLines` shortcut, because the report is about what a
user's wheel does), and compares the DOM renderer's own painted rows (the text of the `.xterm-rows` children) against
what `term.buffer.active.getLine(viewportY + i)` says belongs at each screen row. That observable matters: the buffer
is always right, so the suite's existing helpers, which read the buffer, could never have shown this bug. A "split" is
a sustained disagreement for part of the viewport; the check requires three consecutive 300 ms samples to disagree
before failing, so ordinary one-frame paint lag does not count.

Five shapes:

1. Plain flood, scrolled back before scrollback fills (a paced producer, 2 ms per record, so "not yet full" can be
   asserted before scrolling).
2. Plain flood, scrolled back after scrollback is already full (polls `baseY` to 12,000 first).
3. Output confined to a DECSTBM scroll region with a fixed banner above it, scrolled fully past the active screen.
4. Shape 3 with each chunk bracketed by DEC 2026 synchronized output.
5. A plain flood while a sibling band (the tab close-confirmation row) is toggled six times, forcing re-fits; the test
   asserts a real `term.rows`/`term.cols` change happened and that the viewport is still scrolled back afterwards.

Two fixture facts were learned the hard way and are worth keeping:

- A DECSTBM region whose top is below row 1 never feeds scrollback: `buffer.active.baseY` stays at 0 no matter how
  much is written inside the region. This is standard terminal behavior (`BufferService.scroll` takes its
  `shiftElements` branch when `scrollTop !== 0`). The region fixture therefore has two phases: 300 plain lines first,
  to build real scrollback to scroll back into, and only then the banner and region. The earlier single-phase version
  could not satisfy the "scrolled back" premise at all.
- An unpaced producer (the existing 800,000-record flood) is the wrong instrument for shape 2. With scrollback full
  and the user scrolled back, every evicted line decrements `ydisp`; a full-speed flood walks the whole 12,000-line
  `ydisp` down to its floor of 0 within tens of milliseconds, after which the viewport shows "whichever line is
  currently oldest", changing on every write faster than any repaint cadence could track. That is a more extreme
  condition than the reported one (a user scrolls to a chosen place and it goes stale) and it made the shape pass or
  fail on host load alone. The paced producer keeps evictions at a human-observable rate.

## What was observed

With no workaround present, shape 2 failed on both Chromium and WebKit in recorder runs: the rendered rows showed
records thousands behind what the buffer held at the same screen positions, and stayed there for the whole hold.
`viewportY` sat at the floor the per-line decrement clamps to. That is the reported symptom, reproduced.

Shapes 3 and 4 also failed before the workaround, but their fixture changed afterwards (the scroll offset was
widened to clear the active screen entirely, and pacing was corrected), so those early failures are not clean
evidence about the mechanism on their own. Shapes 1 and 5 never reproduced a split.

After the workaround, all five shapes pass on both engines, and three consecutive full recorder runs plus the
existing flood suite stayed green.

## What is established, from the vendored bundle

Two facts are verified by reading the exact symbols in `crates/farhelm-ui/assets/vendor/xterm.js`:

- `BufferService.scroll`: when the buffer is full (`isFull`) and the user is scrolled back (`isUserScrolling`), it
  runs `ydisp = Math.max(ydisp - 1, 0)` for every evicted line. Otherwise it increments `ybase`, and `ydisp` with it
  when the user is not scrolled back.
- The parser's per-write repaint (`InputHandler.parse`'s epilogue) maps the dirty screen rows into viewport rows by
  adding `(ybase - ydisp)` and requests a refresh only while that offset is below the row count. A row further from
  the active screen than the terminal is tall is never asked to repaint by that path.

Both are correct behavior taken alone. Under a fully scrolled-back viewport the lines it shows do not change (the
circular buffer's window and `ydisp` move together), so "not asked to repaint" is the right answer there. Their value
is negative: they rule out the per-write path as the thing that keeps a scrolled-back viewport honest.

Two more bundle facts were confirmed by the reviewers and shape how the workaround interacts with xterm:

- User wheel scrolls and programmatic `scrollLines` both end in a full-viewport refresh
  (`onRequestScrollLines` handler: `scrollLines(...)` then `refresh(0, rows - 1)`).
- Turning DEC 2026 off is itself a full-viewport refresh request; while the mode is on, `RenderService.refreshRows`
  buffers every request, including the workaround's own. So synchronized output cannot make the split worse; shape
  4 pins that the workaround and xterm's flush-time repaint coexist.

## What was believed and withdrawn

The first explanation, written into the code and the commit message during the delegation, was that
`BufferService.scroll` fires its scroll event synchronously per line, that `Viewport._sync` writes the scrollable
element's `scrollTop` each time, and that the browser coalesces those writes into far fewer native `scroll` events,
starving the refresh that those events drive. A fresh-context review checked this against the bundle and it does not
hold: xterm 6 uses a synthetic scrollable (`SmoothScrollableElement`) with no native scroll listener anywhere in the
bundle, its `onScroll` fires synchronously from `Scrollable._setState`, and `Viewport._handleScroll` computes
`round(scrollTop / cellHeight) - ydisp`, which is zero on the eviction path, so it drives no repaint at all. The
narrative was removed from the code, the spec, and the commit message. It is recorded here only so nobody rediscovers
it as a fresh idea.

## What is unknown

The exact xterm-internal step that leaves the DOM holding stale content, as opposed to merely not being told to
repaint, is not established from source. The second reviewer's reading found no mechanism that produces the split
from the bundle alone: under a fully scrolled-back viewport the lines do not change, the region shapes scroll past
the active screen onto static lines that the user-scroll refresh already painted, and shape 4 gets a full repaint per
bracket. The one derivable case where a scrolled-back viewport's content changes with no repaint request is the
`ydisp === 0` floor with `ybase >= rows`, which the paced shape 2 does not reach in its 3 s hold. So there is a gap
between "reproduced on two engines" and "explained", and the code says so.

Candidates for a later, deliberate look, none of them tested:

- The DOM renderer skipping row updates when `ydisp` changes without a refresh request (instrumenting
  `DomRenderer.renderRows` against `ydisp` changes would settle it).
- Whether the `expectStableViewport` hold in shape 2 is actually observing the floor case some of the time (the
  informational `viewport-y-drift` annotation the test records per run would show it).
- Whether a change-triggered refresh (refresh only when `ydisp` moved since the last paint) would replace the time
  throttle with something both cheaper and more precise.

## The workaround

`refreshIfScrolledBack` in `terminal.js`, called from `writeBytes`'s write-completion callback (which already sits
behind the island's `alive` guard):

- If `buffer.viewportY === buffer.baseY` (following the tail, the overwhelming common case, and trivially true on the
  alternate screen), do nothing. This is the whole hot-path cost: one buffer getter and two integer reads.
- Otherwise issue `term.refresh(0, term.rows - 1)`, throttled to once per 50 ms. The value is a judgment call, not a
  measurement: about three display frames of worst-case staleness on a viewport whose buffer content is static,
  against a full-row DOM rebuild per refresh.
- A write that lands inside an open window arms one deferred refresh (`armTrailingScrolledRefresh`) for the
  remainder of the window, re-checking `alive` and "still scrolled back" when it fires, so output that stops
  mid-window still gets its last repaint. At most one timer is pending at a time; it is cleared in `disposeDeferred`
  with the island's other deferred work. Without this trailing edge the throttle would have reproduced the report's
  own "recovers only after output stops" shape, bounded to the final window.

A test hook, `scrolledRefreshCount`, counts refreshes actually issued. The spec asserts it advanced during each hold
for shapes 1, 2, 3, 4 and 5, so a green run proves the workaround engaged and output was still arriving, not merely
that the viewport happened to stay correct. The docstring on the stability check records what it does not catch: a
throttle silently lengthened to a few hundred milliseconds would still pass, because three consecutive 300 ms samples
pin "no staleness lasting about a second", not the 50 ms constant.

## Process notes

The investigation, fix, and spec were delegated to a sonnet-5 sub agent under the two-checkpoint protocol, then
reviewed three times by fable at high effort in fresh context. The first review is the one that mattered: it caught
the withdrawn narrative, the missing trailing edge, and three unasserted test premises. The second and third rounds
were documentation accuracy and premise wording. Recorder run ids for the failing pre-fix runs and the passing
post-fix runs are in the session's private evidence, not in this repository; the retained run directories live in the
recorder's state root on the development machine.
