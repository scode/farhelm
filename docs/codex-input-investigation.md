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
