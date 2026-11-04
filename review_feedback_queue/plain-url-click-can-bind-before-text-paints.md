# A plain URL click can bind before its text paints

Reviewed commit: ca9b0a19856dc136f63ad2d14f1076027edce1eb

## TLDR

A click can discover a URL in the new terminal model before that text has painted, opening a target the user has not yet
seen. Chromium event batching supplies the conditional source route; the displayed-target guarantee and general redraw
allowance leave coverage ambiguous.

## Details

Suggested bucket: **highest**. Confidence: **source-confirmed defect or conditional mechanism**. confirmed conditional
source route; native occurrence and exploitability unmeasured

Source locations: 7509285e647ea86ccd2cee7905675dc87dad9b6f:crates/farhelm-ui/assets/grid-terminal.js:1266-1301.

Preserve g159/f1. Captured-row identity guards correctly reject changes after capture but do not fence initial lookup
against last paint. The corrected prepaint fixture coordinates establish neither paint nor a whole-finding refutation.
Retain SPEC2327–2331 versus3966–3969 ambiguity.

R grid-terminal.js:1884-1905 commits input frames to the model and schedules paint;570-575 runs that paint only from
requestAnimationFrame. A mouse move at1720-1726 performs lookup against this newer model;1266-1301 asks the current
provider,1329-1341 captures the new model rows as hover evidence, and1671-1718 stores that link on primary press. On
release1608-1612 the link is accepted because both the saved link object and those model rows are current. Neither guard
binds to the last drawn rows. Paint may even occur between down and up: refreshHover1374-1384 skips active gestures, and
painting does not invalidate the pressed link. The caller terminal.js:1339-1347 answers synchronously for ordinary
non-containerized sessions;1299-1313 shows no ordinary plain-URL disclosure;1233-1252 allows that raw open. At T
terminal-links.js:135-140 opens a valid http(s) URL for an admitted click with no selection and mode other than
program;51-56 opens it synchronously. A plain primary press with no modifiers sets an empty selection at T
grid-terminal.js:1116-1121, so selection does not block this route. No OSC 8 or tunnel is required. Native scheduling
source: T e2e/package-lock.json:13-20,42-61 pins Playwright1.64.0; official
https://raw.githubusercontent.com/microsoft/playwright/v1.64.0/packages/playwright-core/browsers.json identifies
Chromium156.0.8078.4. Its official tag resolves to Chromium commit f842f52ef39fd97b449eb27cff414bf05a98ddce. In
third_party/blink/renderer/platform/widget/input/main_thread_event_queue.cc at that commit, DispatchEvents635-665
dispatches queued events through the last non-rAF-aligned event in one main-thread call; QueueEvent792-850 posts that
call for nonaligned input, and IsRafAlignedEvent891-910 classifies mouse moves as optionally aligned while button
down/up use the nonaligned default. Thus an ordinary pending mouse move followed by down can run together before the
queued paint, selecting and pressing the new target while the prior URL remains drawn. The release need not precede
paint. This removes the initial record's overly strong missing premise that the entire click must happen before the next
frame. Retrieved source and tag response are retained privately as chromium-156-event-queue.cc and
chromium-156-ref.json. Earlier Chromium142 source at commit4b8153ab58d3c3f4c9f7e4baad9616ecf80db5fa independently has
the same queue rule; it is supplemental, not the test-dependency basis. Tip persistence: R-to-T diff changes
selection-boundary rounding and input-report helper calls, but does not repair the link/model/paint ordering. At T the
relevant lines are grid-terminal.js:1272-1307,1380-1390,1590-1618,1677-1732,1890-1911. Caller
terminal.js:1233-1252,1299-1313,1339-1347 remains the same on this route.

A user clicking a printed terminal URL can open a replacement URL the program has supplied but the terminal has not yet
drawn. The ordinary plain-URL route has no separate destination disclosure.

Possible coverage and caveats: T SPEC.md:3966-3969 accepts sub-second actions before redraw but preserves named guards;
T SPEC.md:2327-2331 expressly dispenses with ordinary URL disclosure because displayed text is what opens. The
interpretation of that explicit guarantee against the broad timing allowance should remain visible to triage. T
TRIAGE_OUTCOMES.md:18817-18857 confirms named guards hold across render/paint gaps but ordinary target selection outside
named guards may follow delivered input. Its queued-launch-capture-before-repaint.md decision16402-16417 concerns
template edits without a wrong-host/folder launch, not plain-URL disclosure. Its
approval-cards-can-obscure-file-disclosure.md decision15575-15617 preserves no-hover plain-URL behavior, not choosing an
undrawn replacement. Historical vendored-xterm partial-repaint outcomes7680-7721 and TODO.md:1033-1036 are not coverage
for this first-party grid code. Planned bucket is empty(TODO.md:23-25); searches of SPEC.md, SPEC_impl.md, TODO.md,
unlanded plans/queue, BUGS.md, queue INDEX and TRIAGE_OUTCOMES.md found no exact grid prepaint-opening acceptance or
planned repair. FILTER.md's race filters exclude security consequences.

Additional reviewed locations: g159/f1:
crates/farhelm-ui/assets/grid-terminal.js:1272-1308,1380-1390,1614-1618,1690-1707,1890-1911.

Provenance: private review run `37478d09-7e36-409b-8efa-b13d1f793dd5`; original identities `g160/f2`, `g159/f1`.
Original append-only events, revisions, withdrawals and independent judgments are retained in that run's catalogs.
Locations refer to the named reviewed revisions; the original revision context is
7509285e647ea86ccd2cee7905675dc87dad9b6f (R). Persists at inclusive tip ca9b0a19856dc136f63ad2d14f1076027edce1eb (T)..
This was a source-only review; runtime occurrence and frequency were not measured.
