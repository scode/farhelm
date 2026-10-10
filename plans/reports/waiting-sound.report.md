## What this was about

An agent waiting for an answer could go unnoticed while its user looked elsewhere. The maintainer chose three distinct
sounds, separate device switches, quieting for the active open session, and coverage of sessions hidden by filters.

The change adds a bell when a session starts waiting, a ding-dong for a new Farhelm approval request, and an optional
soft pluck when an agent goes from running to idle. Waiting and approval default on; turn finished defaults off.
Settings remembers each switch on this device.

## Things you should know

The first authenticated read is silent, including requests already waiting. The session open in an active window stays
quiet. Hidden sessions still count. A burst plays only the most urgent enabled, non-quiet event: approval, waiting, then
turn finished. Only Claude and Codex report Waiting today; the change does not extend status detection to other agents.

Browser playback requires interaction with the page. Blocked sounds are discarded, never replayed later. Each open
client makes its own sounds. Detection keeps the helm's existing 500-session list cap.

Linux WebKitGTK 2.52.6 scheduled the shipped waiting sequence through real Web Audio without a gesture under the
autoplay-allow policy used by the pinned desktop webview library. This was an engine-policy probe, not a test through
Farhelm's native window. This machine has no physical audio device, so the sounds were not auditioned. macOS playback
and device-storage persistence were not checked; persistence uses the existing terminal-text-size webview storage
assumption.

## Open questions and possible follow-ups

No implementation decision is awaiting an answer. Listening on a machine with audio, and checking macOS native playback
and persistence, remain useful platform checks.

## PRs

- [#1768](https://github.com/scode/farhelm/pull/1768/changes): device sounds for waiting, approval requests and optional
  finished turns, with Settings, specification, docs and tests. Draft head `320d9b7962cb7c8443d05ee9d9f4d3d24edb2bd5`.

## Checks run, reused and skipped

The final implementation was tested at `4944f6e3c33c52910af2fe694dffe5e331ec89f6` before publication. The careful rebase
onto `a259d600` preserved every actual edited line; upstream added unrelated TODO entries and queue bookkeeping only.
The later sweep-plan delivery on main also changed only its queue state and report. The pushed head therefore reuses the
checks below without a runtime rerun for those metadata changes.

- `cargo build`, release `dx build --package farhelm-ui --platform web --release`,
  `cargo check -p farhelm-ui --features desktop`, and `cargo clippy -p farhelm-ui --all-targets -- -D warnings` passed.
  The web and desktop checks were repeated after the registration and reader-observer corrections. These cover the
  changed UI configurations.
- Recorded focused UI Rust tests,
  `cargo nextest run -p farhelm-ui --lib -E 'test(feed::tests::) | test(reader::tests::)'`, passed 18/18 in run
  `95bce74b-df92-4923-95f0-0fcdd9380eff`; 423 unrelated cases were selected out. Pinned nextest 0.9.143 enforced four
  slots and zero retries. These pure state tests use no tmux substrate and printed no runtime substrate skips.
- Recorded UI JS discovery, `cd crates/farhelm-ui/js-tests && node --test`, passed 213/213 in
  `c6832aae-0db1-42d1-8888-69dc47cc4268`. After adding the storage-write-refusal case and strengthening the quiet-rule
  table, the focused `node --test sounds.test.js` passed 11/11 in `ab51770c-3ec6-41d1-9d75-5c9be31ea649`. The other
  harness inputs and shipped sound asset were unchanged, so their earlier coverage remains applicable.
- Recorded Playwright sound wiring and the real denied-approval-card case passed 8/8 across Chromium and WebKit in
  `681cc7fd-17b4-4bd1-9291-1251ef3caf13`. Selected feed cases passed 10/10 in `9a1b1435-ad88-42fd-a05c-0b4400ba65fd`:
  queued work and selection lifecycle, healthy silence, outage polling/recovery, handshake attribution and build-skew
  withdrawal. Both runs enforced one worker and zero retries, with pinned tmux 3.7c.
- Initial browser run `9be96506-222a-490b-b4fa-698411594425` had 10 passes and two failures: the existing Settings
  isolation assertion still expected two checkboxes after three sound switches were added. Its failed evidence remains
  retained; the count was corrected to five while preserving focus and modal assertions. The exact case then passed on
  both engines in `573ad076-cb5b-4156-9978-024eafdf1e35`. The initial run's other Settings cases remain applicable
  because their code did not change. This was a same-session fixture correction, not a latent-flake entry.
- Recorded `scripts/check-desktop-assets.sh` passed in `3b0684ce-4e77-4834-a156-a42939b12ebc`: all 22 requested assets
  match the bundle, including the sound script. Both deliberate divergence checks also passed.
- Recorded Linux WebKitGTK autoplay probe `10a975ad-e3bb-436b-8db5-d1fb4efc7e9a` used the shipped asset and real Web
  Audio. It observed a running context and the six approved waiting pitches without a gesture. Its native-window and
  hardware limits are stated above.
- `cargo fmt --all -- --check`, `dprint check`, changelog format and the isolated
  `python -B scripts/check-test-sleeps.py` passed; the last inspected 276 delays with zero missing rationales.
  `dprint check TODO.md` passed again after rebase. The website frozen install and build passed with 32 pages; its
  sources were unchanged by later fixes, so that result is reused. The two Settings screenshots were visually inspected
  with all switches visible and no clipping.

A portable retained-run summary was archived privately for nine explicitly named runs; discovery was complete. Generic
recorded runs do not provide structured case counts, so their counts above come from retained console output rather than
an inferred aggregate.

The full Rust and browser batteries, workspace-wide Clippy, desktop smoke, installer, provisioning and release checks
were skipped: the selected UI, reader, approval and engine checks address the concrete risks, and no corresponding
backend, shell or release behavior changed. Doctests were skipped because no executable documentation changed. No hosted
CI or website deployment was requested. macOS, physical listening and full native-window autoplay integration remain
unchecked as stated above.

## Review gate outcome

The required fresh-context gpt-6.1-sol high review covered general correctness, design, language idiom, the full
test-authoring checklist, and scope. Its hidden-tab finding was fixed by replacing an animation-frame registration wait
with the existing timer pattern. A follow-up found an incorrect test-helper release call; both held requests are now
released explicitly. The reviewer verified both corrections and reported no remaining findings or unnecessary
complexity.

Commit and PR wording passed a fresh gpt-6.1-sol medium cold read. The delivery report has a separate fresh native cold
read before submission.
