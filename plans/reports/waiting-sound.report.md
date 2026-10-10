## Since the last review

The existing draft #1768 was revised; no PR was added or dropped. The maintainer replaced fleet-wide status sounds with
sounds for the sidebar’s current view. The revision removes the separate session-list request and silently establishes
status history when a different host or filter is accepted. Approval requests still sound across all hosts. It also
unlocks audio on trusted touch release or click, clarifies shared and per-device Settings, and gives the storage-refusal
test an audible positive control.

## What this was about

An agent waiting for an answer could go unnoticed while its user looked elsewhere. Farhelm gains a bell when a visible
session starts waiting, a ding-dong for a new approval request, and an optional soft pluck when an agent goes from
running to idle. Waiting and approval default on; turn finished defaults off. Settings remembers each switch on this
device.

## Things you should know

The first accepted view and existing approvals are silent. Changing hosts or filters silently establishes the new view’s
baseline; subsequent observed transitions can sound, even if an approval reply was delayed across the change. Sorting
and reconnecting preserve history. The session open in an active window stays quiet. Only Claude and Codex report
Waiting today.

Each observation plays the highest-priority enabled, non-quiet event: approval, waiting, then turn finished. Independent
replies may produce separate sounds; this is not a timed burst window. Status coverage follows the sidebar’s listing
cap. Approval requests remain eligible even when their session is hidden.

Browser playback requires a trusted interaction. Touch release, click and keyboard input attempt to enable audio;
refused sounds are discarded and never replayed. Each open client makes its own sounds.

The real-browser touch check deliberately suspends an actual AudioContext and observes trusted emulated-touch resumption
and oscillator scheduling. It does not establish physical-phone autoplay policy. There is no physical audio output
device on the test substrate, so the sounds were not auditioned. The previous Linux WebKitGTK 2.52.6 probe observed
playback scheduling without a gesture under the autoplay-allow policy used by the pinned desktop webview library; it was
an engine probe, not a Farhelm native-window test. macOS native playback and device-storage persistence remain
unchecked.

## Open questions and possible follow-ups

No implementation decision awaits an answer. Auditioning the sounds, checking physical mobile-browser activation, and
checking macOS native playback and persistence remain useful platform checks.

## PRs

- [#1768](https://github.com/scode/farhelm/pull/1768/changes): device attention sounds, Settings, specifications,
  documentation and tests. Draft head `fea036f69f62ae18cb55c29283e7976dfe7541d3`.

## Checks run, reused and skipped

Current desktop UI compilation, `cargo build`, release `dx build --package farhelm-ui --platform web --release`, and
`cargo clippy -p farhelm-ui --all-targets --features desktop -- -D warnings` passed. The frozen website install/build
produced 32 pages with valid internal links. These checks cover the changed UI configurations and documentation.

Recorded full UI JavaScript discovery (`cd crates/farhelm-ui/js-tests && node --test`) passed 217/217 in
`e1580183-a5c2-4b2a-90ae-e1b5ca386a63`, including trusted-release/click unlocking and the enabled-audio storage-refusal
control.

All seven Chromium sound scenarios passed in `5b6aebb8-25b1-46a8-97f4-fa1b93a3fd5a`; that same run had seven WebKit
launch failures due to missing compatibility libraries. An earlier run `7f64a7aa-f622-4100-8b61-4234abdb94ea` could not
launch either engine because browser dependencies were absent. Both failed records remain retained. Sandbox
prerequisites were corrected; exact Chromium and WebKit scenario probes passed in `c08e377a-5e5e-4b12-87e2-11de4f5d5ca8`
and `7b94e192-ac5c-4330-a84b-e6e0709a0dcd`. These were substrate failures, not product assertions.

The seven sound scenarios passed on WebKit in `7e3b438a-a7eb-4a8b-b1f1-ca627bb1247a`, completing both-engine coverage of
the revised wiring. This includes current-view quieting and reveal, approvals outside the view, Settings persistence,
active-session silence, first-load silence, delayed initial and changed-view baselines, returning-view silence, and real
AudioContext touch resumption. The Settings screenshot was visually inspected with every switch and Close visible.

Recorded selected integration cases passed 26/26 on Chromium and WebKit in `82d1ab16-e9b3-4e21-ab49-cbecf5852196`: sort
persistence and preference seeding, host/filter and sort interaction, held or failed listings, stale pre-stop replies,
queued reader work, healthy-feed silence, fallback/reconnect, handshake attribution, build-skew withdrawal and Settings.
Both-engine recorded runs used one worker, zero retries and pinned tmux 3.7c; the exact recovery probes and the
WebKit-only sound completion used the generic recorder with the same runner budget.

Current `scripts/check-desktop-assets.sh` passed in `e0d3d1e4-a989-4798-bfe5-09e11c2faf66`: all 23 requested assets
matched the web bundle, including the sound script; both deliberate divergence controls passed. A portable retained-run
summary was archived privately. Generic records do not provide structured case counts; their counts above are from
retained output.

`cargo fmt --all -- --check`, changelog format and the isolated `python -B scripts/check-test-sleeps.py` passed; 278
deliberate delays have rationales. `dprint check` passed before the rebase, and checking the touched documents and
browser/JS files passed afterwards. Later source-review changes were only test indentation.

The implementation was tested on main `e1ea1929`, with the second-round sidebar correction applied. The final careful
rebase onto `8bc1af9a` preserved every reviewed substantive changed line; later edits were test indentation and the
changelog’s scope wording. Earlier upstream changes concerned BusyBox host support, remote-uninstall wording, queue
records and collected review notes. The final upstream code change tightened OMP conversation ownership when a custom
Bun/npm launch has unreadable top-process arguments, retaining the installed-OMP exception. That guard changes Resume
admission, not sound statuses, approvals, accepted listings, feed inputs or audio assets. Its spec section was preserved
beside the sound section. Each successful build, JavaScript, lint, website, sound, integration and asset check remains
applicable; no additional runtime run was justified by those independent changes.

The earlier pure reader/feed unit run `95bce74b-df92-4923-95f0-0fcdd9380eff` passed 18/18. Those reader/feed
implementations and test inputs are unchanged; this is reusable evidence of their state machinery, not proof of the
revised sidebar wiring. Earlier unfiltered sound browser runs are not reused for that new wiring. The prior Linux engine
probe `10a975ad-e3bb-436b-8db5-d1fb4efc7e9a` remains evidence of unchanged note generation and autoplay policy only,
with the limits above.

The full Rust and browser batteries, workspace-wide Clippy, desktop smoke, installer, provisioning and release checks
were skipped: focused sound, Settings, feed, listing and asset checks cover the concrete changes, and no backend,
installer or release behavior changed. Doctests were skipped because no executable documentation changed. No hosted CI
or website deployment was requested.

## Review gate outcome

The required fresh-context gpt-6.1-sol high source review covered correctness, design, language idiom, scope and the
full test-authoring checklist. It found that delayed approval replies could hide later waiting transitions in a newly
accepted view, including the first load. The revision now preserves the first accepted statuses for each view and
initializes approval history independently; the reviewer accepted the corrections with no remaining findings or
unnecessary complexity. It also accepted retaining the existing bounded approval reader and documenting priority per
observation rather than adding a timed coordinator.

Commit and PR wording passed a fresh gpt-6.1-sol medium cold read. The delivery report receives a separate fresh native
cold read before submission.
