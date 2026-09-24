# Codex input-area corruption investigation

The report remains unresolved. This investigation has not established a Farhelm defect or reproduced the reported
scattered display fragments. A clean observation does not establish that the input or rendering path is always correct.

Observed on 2026-09-08. Source inspection started at `722a690f4ee06dbf7350519811ddec0bcc3e5413`; runtime probes used a
development build reporting Farhelm `0.5.0-rc.3`, with the concurrent host-selector simplification applied in progress.
The tested CLI SHA-256 was `233dd5cf70cfade43397e5b519a6f179665bf32ad38db83ad81421cf66e62854`. The operator retained the
exact CLI/web artifacts, private diagnostic fixtures, screenshots, and recorder evidence under the run IDs below. These
exploratory fixtures are not maintained regression tests.

## Report and scope

The reported client was the macOS desktop app, with a local helm and an agent on a remote Ubuntu machine. Quickly
appending `include a SPEC.md` while editing a prompt left `include a SPE` and scattered `COMMI`, `PR`, and repeated
`SPEC` fragments separated by large gaps. Whether the submitted input would have been correct is unknown: the symptom
appeared before submission. No incident screenshot, logs, exact Codex version, or exact Farhelm build was supplied.

The investigation used Codex CLI 0.153.4 in an owned Ubuntu 26.04 container, with pinned tmux 3.7c, a synthetic Codex
home, and an empty trusted working directory. It did not read existing conversations. Linux browser WebKit can provide
engine-family evidence, but it does not exercise macOS WKWebView, the desktop eval bridge, or the original network.

## Observations

Direct Codex with `--no-alt-screen` inside tmux, without Farhelm, showed the complete prompt at 120, 80, and 50 columns
after both individual key injection and a whole-text burst. These are different stimuli; the burst is not evidence about
keyboard event handling in a browser. The six observations in run `df948442-c672-4dc9-bf69-baef5362c8c5` followed model
selection becoming visible, but MCP startup text remained on screen. They do not establish a fully idle composer or
exclude a transient frame between captures. No prompt was submitted in these composer probes.

An earlier six-case run, `507714c5-d66d-450d-a50d-94ce669e13a7`, also showed intact input, but the model was still
loading; it is startup-only evidence. Run `078211b4-60b1-4509-87df-d788a24ace94` failed while creating the third probe
session. A last-session tmux teardown race was suspected, but the failing subprocess's stderr was not retained, so that
cause remains a hypothesis. The run retained no completed-case output and contributes no successful observations. The
later run succeeded after adding an idle keeper pane to the fixture.

A second six-case comparison, `d8d2993c-5464-4835-9df4-d610848a1fcf`, omitted `--no-alt-screen` and used a longer
synthetic prompt containing `SPEC`, `COMMIT`, and `PR` before the exact suffix. All six retained tmux screens contained
the whole prompt after whitespace normalization for wrapping. Individual-key injection took 1.28–1.70 seconds for the
whole prompt; a whole burst took 4–7 milliseconds. MCP startup was still visible in the burst observations, but absent
in the three individual-key captures. These snapshots do not measure intermediate frames or browser keyboard events.

WebKit run `9d0dc368-8614-4de9-bff6-bc37defeeed5` exercised the web build against a local helm and a second supervisor
reached through loopback SSH. A short prompt at 1440 pixels and the longer prompt at 1000 pixels each received the exact
suffix through browser keyboard events with no programmed per-key delay. xterm's buffer and screenshots showed the
complete prompt before and after reducing each viewport width by 80 pixels. The retained state was live and revealed,
with replay complete. The resize followed typing; this does not establish behavior under simultaneous resize and input.
No browser paste path was tested. The Linux engine was Playwright WebKit 26.5 through Playwright 1.62.0.

Chromium run `550c3463-8e9e-4c84-a511-53ab94edae70` repeated both prompt widths and subsequent resize using Chromium
151.0.7922.34 through Playwright 1.62.0. The current prompt was complete in both buffers and all four screenshots.
Across both engines, all eight composer screenshots showed the complete current input. These were asynchronous
snapshots, not a recording of every redraw.

The captures were not uniformly tidy: Chromium's long-prompt buffer after resize retained startup frames and an earlier
partial prompt row in scrollback, followed by the complete current prompt. Its screenshot showed the complete current
input without scattered fragments. WebKit's short-prompt resized screenshot had awkward banner word wrapping, while the
input remained complete. These observations do not establish pristine scrollback or eliminate transient rendering
defects.

## Source trace

The ordinary open-socket path preserves accepted input ordering: `terminal.js` encodes xterm's `onData` text, the helm's
terminal reader awaits each send, the supervisor accepts frames under the current attachment's ownership, and the tmux
input client sends hexadecimal bytes and waits for tmux's acknowledgement. The relevant code is
[`terminal.js`](../crates/farhelm-ui/assets/terminal.js), [`terminal.rs`](../crates/farhelm-helm/src/terminal.rs),
[`connection.rs`](../crates/farhelm-supervisor/src/service/connection.rs), and
[`input.rs`](../crates/farhelm-supervisor/src/tmux/input.rs). This trace does not prove which bytes reached Codex during
the reported incident.

`sendData` sends only when its browser WebSocket is OPEN; it does not queue typing across a closed or connecting socket.
That is a concrete loss window, but no evidence places the incident in it, and dropping an input chunk alone does not
explain unrelated screen fragments. Treating this seam as the incident's cause would overstate the evidence.

Output rendering is separate. xterm consumes cursor-addressed terminal output; replay, container resize, and font
settlement can change its display and cell geometry. The existing code orders replay before the completion marker and
fits the terminal after container or font changes. Source inspection cannot exclude a timing-dependent interaction among
Codex redraws, tmux, xterm reflow, and WebKit painting. None of those possibilities has been demonstrated here.

## Next useful evidence

At a recurrence, compare xterm's buffer and pixels before forcing a redraw, and retain the socket generation/state,
replay state, terminal dimensions, and desktop stderr timeline. A correct buffer with incorrect pixels would narrow the
issue to painting; incorrect input bytes would instead direct investigation toward input delivery. A short bounded trace
at xterm input, WebSocket send, helm receive, and the owned PTY can distinguish those paths without reading an unrelated
conversation.

Follow the [desktop/web triage guide](desktop-web-triage.md) to compare the desktop app, Safari, and Chromium against
the same helm. The exact running versions and launch configuration are needed to connect that result to a vendor or
Farhelm change. Until then, the TODO stays open.

## Follow-up on 2026-09-23

The current source makes a transport-level reorder less likely than the original note did. `terminal.js` sends each
xterm `onData` event as one binary WebSocket message. The helm's inbound loop reads those messages in order and awaits
each supervisor `send_input` call before reading the next one. The supervisor's dedicated tmux input client writes
`send-keys` commands in order and waits for each command reply. There is still a loss window when the socket is not
open, but there is no evident concurrent writer or unordered queue that would turn a suffix into repeated, separated
words.

The more plausible Farhelm boundary is painting. xterm.js 6.0.0 maintains a correct terminal buffer while its DOM/canvas
rows can become stale. Farhelm reproduced and fixed one form of this on 2026-09-12: a terminal scrolled into scrollback
could show old rows while new output continued, leaving the rendered rows different from `term.buffer.active`. Codex is
a particularly good stimulus for this class of failure because it continuously redraws a prompt with cursor movement
while the user types. The existing fix forces a throttled full-row refresh only when xterm reports that the viewport is
scrolled back; it does not prove that the active-tail path in the macOS WKWebView is sound.

There is independent vendor evidence for both halves of the hypothesis. OpenAI Codex issue
[#46024](https://github.com/openai/codex/issues/46024) reports severe block-pattern corruption in Apple Terminal during
macOS dictation even though the submitted conversation still works, which is a visual corruption pattern with intact
underlying input. Issue [#32691](https://github.com/openai/codex/issues/32691) reports dropped characters and raw CSI-u
fragments in iTerm2, tying genuine input corruption to terminal keyboard-enhancement handling. Neither issue proves a
Farhelm defect, and neither exercises Farhelm's WKWebView or tmux path.

The earlier Farhelm probes cannot distinguish these cases: they sampled completed frames after typing, did not capture a
native macOS webview, and did not exercise IME/dictation or a paste event. At the next recurrence, capture both
`term.buffer.active` and the rendered rows before forcing a resize or switching sessions. If the buffer contains the
exact prompt while the pixels contain the scattered fragments, the defect is in xterm/WKWebView painting. If the buffer
itself is wrong, add a bounded trace at xterm `onData`, WebSocket send/receive, and the tmux input confirmation to
identify the first divergent byte. This is the shortest experiment that can separate the two live hypotheses.

## Reproduction update on 2026-09-23

The report has now recurred on Farhelm `0.14.0-rc.5` while typing `SPEC.md` quickly. A supplied capture shows the
terminal line remaining correct through `SPE`, then displaying separated `SPEC` and `SPECIAL` fragments at different
columns. The text before that point was deliberate random input and is not part of the symptom. This shape is compatible
with stale cursor-addressed painting, but this occurrence also establishes an input fault: the operator typed the line
in Farhelm, pressed Enter, and the same malformed text arrived here from the Codex agent. The screenshot and submitted
text match, so the malformed bytes crossed the terminal input path rather than existing only in the viewer's paint.

The `SPE>MD` shape adds one useful clue: on a US keyboard, `>` is the shifted form of `.`, while `C` is also a shifted
letter. If the intended text was `SPEC.md`, a stale or misordered Shift transition could explain both a missing `C` and
a period arriving as `>`. This points at the native key-event/composition path rather than tmux or TCP, although the
separated `SPEC`/`SPECIAL` fragments could still include a concurrent redraw artifact.

There is now a close upstream match in xterm.js. [Issue #6078](https://github.com/xtermjs/xterm.js/issues/6078)
documents that xterm's hidden textarea retains uppercase letters and spaces, then re-emits the accumulated value through
`onData` when a `keyCode`-229 composition-character event changes that textarea. Its `String.replace` diff treats an
in-place edit as new input, so previously typed capitals can be injected again. Farhelm's `terminal.js` forwards
`onData` directly and the helm and tmux paths preserve those bytes, which explains how duplicated `SPEC`-like text can
reach Codex. The vendored xterm.js is 6.0.0; the upstream report reproduces on later 6.1.0 beta builds as well, so this
is not tied to one Farhelm release.

[Issue #5894](https://github.com/xtermjs/xterm.js/issues/5894) describes a second WKWebView composition defect: a dead
key's committed character is emitted twice and the following physical key is lost. That report's `SPE>MD` pattern is
consistent with the missing `C` and shifted period clue above. These two xterm defects can coexist in the same macOS
input path; the exact one that fires needs an event trace.

The leading hypothesis is therefore an xterm/WebKit composition bug, with terminal-query leakage as a secondary
possibility. `SPEC.md` is a useful trigger because its uppercase letters exercise the hidden-textarea accumulation path
and its period exercises the shifted/unshifted transition. A focused trace should record composition events,
`keydown.keyCode`, the hidden textarea value, and `onData` before tracing the network path.

The next recurrence should be captured at four boundaries in one bounded record: composition/key events and the hidden
textarea, the exact bytes emitted by xterm's `onData`, the binary WebSocket frames received by the helm, and the bytes
confirmed by the tmux input client. A malformed `onData` value proves the xterm/WebKit defect; matching `onData` and
tmux bytes rules out the Farhelm transport. The submitted prompt should also be compared with the captured xterm buffer
before any redraw or session switch.

The related dead-key path has now been reproduced directly: `Option+N` followed by `/` produced `˜˜` instead of the dead
character followed by `/`. That is the same duplicate-dead-character and dropped-next-key result documented by xterm.js
[#5894](https://github.com/xtermjs/xterm.js/issues/5894), which confirms that this Farhelm desktop input surface reaches
the affected WKWebView/xterm path. It does not by itself prove that every `SPEC.md` incident uses the same subcase, but
it moves the leading cause from a general Farhelm transport hypothesis to xterm's macOS composition handling.

The `SPEC` trigger itself is not yet deterministic: the operator has seen the corruption after typing `SPE` without
using the accent picker, but has not found a fixed timing sequence. That does not contradict the xterm diagnosis. Issue
#6078's trigger is a `keyCode`-229 or composition-character event, which can come from IME, dictation, dead-key
handling, or other WebKit input transitions; an accent-picker action is only one way to force such a transition.
Comparing rapid `SPE` with rapid lowercase `spe`, and repeating each after an Enter that clears the textarea, should
distinguish the uppercase accumulation path from an unrelated redraw trigger.

Network timing may change the reproduction rate without being the byte-level cause. WebSocket and TCP preserve input
ordering, and a closed socket can drop input but cannot duplicate or reorder it. Remote output timing can still alter
races between typing, Codex redraws, split terminal-query frames, and browser painting. The direct `Option+N` then `/`
result of `˜˜` is local evidence for the WKWebView/xterm path because it reproduces the known dead-key defect before
Farhelm's WebSocket or tmux delivery.

Another live reproduction on 2026-09-23 submitted `SPECIALLY` after the operator intended to type only `SPE`; the
capture shows the same contiguous word. This is stronger than the earlier separated fragments and fits #6078's retained
uppercase-input mechanism if `CIALLY` was already present in the hidden textarea from earlier typing. The issue does not
invent arbitrary letters, so a clean post-Enter attempt containing only `SPE` is the discriminator: persistence after
that reset would point away from textarea accumulation and toward macOS input-method completion.

## Mitigation attempt on 2026-09-23

Every fragment from the 2026-09-23 recurrences (`SPEC`, `SPECIAL`, `SPECIALLY`) is an ordinary English completion of
`SPE`, which is the signature of macOS inline predictive text rather than of arbitrary retained input. The original
report's `COMMI` and `PR` fragments are not completions of `SPE`; they fit #6078's re-sent textarea better, since that
textarea holds the capitals typed earlier on the line, and this mitigation does not address that path. xterm's hidden
input textarea keeps typed capitals until Enter, Ctrl-C, or blur, so after `SPE` it holds a plausible word for WebKit to
complete, and xterm's `keyCode`-229 handler forwards any growth of that textarea through `onData`. xterm sets
`autocorrect`, `autocapitalize`, and `spellcheck` off on the textarea but not `writingsuggestions`, WebKit's opt-out for
inline predictions. `terminal.js` now sets `writingsuggestions="false"` on it after `term.open()`.

This is a hypothesis-driven fix, not a confirmed one. It has not been exercised on macOS, it depends on the system
WebKit supporting the attribute, and there is no reliable reproduction to test it against. It does not touch the
dead-key defect (#5894) or other routes by which #6078's accumulated textarea could be re-sent. If the corruption recurs
with this change in the running build, first check that `.xterm-helper-textarea` carries the attribute and that
`'writingSuggestions' in HTMLElement.prototype` is true in that webview; if both hold, the prediction hypothesis is
wrong or incomplete and the four-boundary trace above is the next step. Turning off System Settings → Keyboard → Text
Input → "Show inline predictive text" is the equivalent system-wide experiment.
