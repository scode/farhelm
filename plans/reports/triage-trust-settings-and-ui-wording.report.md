# Workspace trust and Settings wording

## What this was about

A session's own sandbox or container can receive automatic workspace trust. Farhelm could append its own trust option
after a user's Custom permission flags, potentially replacing the choice they supplied. The maintainer approved
best-effort detection of those overrides. Settings implied that persistent hosts created with Add host could receive
automatic trust, and screen readers reported unread hook and checkout switches as off. A trust hint could send someone
in their own sandbox to a switch that had no effect there. A Restart approval could credit Settings for trust saved with
the session when it started.

## Things you should know

- Farhelm skips its own opt-in when it recognizes a Custom trust override. It recognizes common Codex project overrides
  as well as Pi's explicit approval choices (`--approve` and `--no-approve`) and Antigravity's user-supplied settings
  (`--settings`). Fresh structured Pi launches currently offer only YOLO, so Pi's detection is covered at the flag
  level. Codex is covered through launch composition both on a host with trust enabled in Settings and in a session's
  own environment. The docs describe the detection as best effort.
- The Settings help names local hosts, SSH hosts and persistent hosts created with Add host. Only a session's own
  sandbox or container gets automatic trust by default. A screen reader now announces the conversation-hook and
  prepared-checkout switches as unavailable while their saved value is unknown. When a read succeeds, the switches
  report the saved state again.
- A trust question can still appear in a session's own sandbox or container. Its hint now waits for an ordinary or
  persistent host record before offering a Settings switch. Agent-requested Restart approvals explain that trust was
  saved with the session's launch. The new dismissal test verifies that a refusal arriving after a dismissal does not
  bring an old error back when the hint is re-enabled.

## Open questions and possible follow-ups

- The warning text is still attached to the trust and hook switches for assistive technology. The existing GUI test
  driver does not expose those descriptions. A direct assertion would need changes to the vendored driver and its
  matching patch, so the Nov 21 small-test rule leaves this coverage gap. The new tests cover the switches' role and
  value while a read is pending or failed.

## PRs

- Best-effort Custom trust detection. [#5043](https://github.com/scode/farhelm/pull/5043/changes)
- Settings trust wording and unread switch announcements. [#5046](https://github.com/scode/farhelm/pull/5046/changes)
- Trust hints and saved-launch wording on Restart approvals. [#5049](https://github.com/scode/farhelm/pull/5049/changes)

## Checks run, reused and skipped

- The initial focused helm run on `c39dd62e` passed 64 of 65 tests. The failing fixture asked for a fresh Pi Custom
  launch, which the compiler correctly refused. The follow-up passed all 25 trust and launch cases after using supported
  Codex composition and retaining the Pi flag cases on `f03b13cd`. Both runs used
  `cargo nextest run --locked -p farhelm-helm --lib -E <selection>` with four test threads and retries disabled. Strict
  helm Clippy passed. Recorder runs `b485db3c-bf90-4645-9b55-b1f2cbccb0fe` and `b1c4873f-5eb0-4a6c-a1da-0f7ba43edf88`.
- On `e75d6625`,
  `cargo nextest run --locked -p farhelm-gui --lib -E 'test(/^settings::tests::|^components::checkbox::tests::/)'`
  passed 78 tests. We ran `python3 scripts/check-gui-goldens.py --update -j 6` first with selected Settings scenes and
  then with the whole gallery. The selected screenshots and full update matched except for the expected copy changes in
  the five agent trust pages and the Antigravity Settings search snippet, in both themes. The twelve changed references
  were reviewed and published. Strict GUI Clippy, Rustfmt and the delay audit passed. Recorder runs
  `d2ef18bb-92e0-4ee6-a3bc-0e1412a13a53`, `1d6e4e39-69ea-4ff2-9e58-385a65b453da` and
  `e460435c-a82d-4c8a-9ce5-88aec1728474`.
- The rebase on `491df5c5` brought in the golden-render deadline and updates to agent-request admission and checkout
  history. On `ae8665db`, `cargo nextest run --locked -p farhelm-helm -p farhelm-gui --lib -E <selection>` covered 171
  cases across workspace trust in the helm, Settings, trust hints, approval cards and the golden-render budget. It
  passed 169 cases. The failures caught an approval fact that compared differently with no trust text and a host test
  that tried to send updates before installing the Hosts collection. The corrected 61-case approval and trust-hint
  selection passed on `f9d9adfa` with `cargo nextest run --locked -p farhelm-gui --lib -E <selection>`. The passing
  helm, Settings and render-budget unit cases from `ae8665db` still apply. Later changes affected only approval facts,
  the host test setup, and the Restart warning layout and fixture. Recorder runs `fa273f83-e635-4632-a3d3-88ece06dc255`
  and `2e00a12e-291f-44bd-8ee1-b34160e0e1c9`.
- The rebased full golden update on `f9d9adfa` matched the twelve Settings references and changed only the Restart scene
  in both themes. It exposed a clipped trust warning, so the warning now wraps inside the card. Strict helm and GUI
  Clippy passed before that layout adjustment. The helm evidence still applies because only the GUI changed. Recorder
  run `9b394279-a1fd-46b0-a8d4-4e84717e4dea`. On `5f52c34b`, strict GUI Clippy and all 42 approval cases passed with
  `cargo nextest run --locked -p farhelm-gui --lib -E 'test(/^approvals::(model_tests|card_tests)::/)'`. Both rebased
  full updates used `python3 scripts/check-gui-goldens.py --update --render-timeout 480 -j 6`. The final run stayed
  within the render deadline and changed only the two Restart pictures. The complete warning was visible in both.
  Rustfmt and the 306-delay audit passed. The published tip adds only the reviewed picture references and archive links
  to `5f52c34b`. Recorder runs `da2bff51-fc19-42df-9aa8-cf58095c1bd1` and `b059594d-1f0b-48f0-8ae6-04b754315190`.
- Live vendor launches and macOS assistive technology were outside these Linux checks. The tests verify Farhelm's launch
  composition and the accessibility role and state it exposes. The workspace suite was left to the landing run because
  the changed paths and their interactions had focused coverage.

## Review gate

Each draft had the plan's independent source review. The Custom trust review tightened Codex table detection and
corrected stale documentation. The Settings review added a failed-read case and clarified Claude's help in the spec. The
final review required positive evidence that a host follows Settings before the hint reads or shows, and a regression
test that covers both the missing and session-owned host cases. The source gate cleared after those findings were
addressed. The later layout correction passed focused approval tests and an independent visual review of the wrapped
warning in both themes. The updated Settings pages passed independent visual review as well.
