# Conversation hooks and workspace trust

## What this was about

The November 24 review found problems in the hook setup and trust controls, in what an approval promised, and in how
Farhelm explains whether Restart can resume a conversation. Someone approving a fresh checkout could see one trust
choice while a later Settings change affected preparation. Screen-reader users could miss the permission warning, and
changing tabs in the launcher could lose an unanswered hook setup offer.

This stack resolves 20 of the 26 outcomes, including the final regression-test outcome. It leaves four polish items
unchanged under their size limits and records two deferred fixes in Plan follow-ups. Every outcome retains its original
decision and its PR link in the triage archive.

## Things you should know

- **Approvals carry the checked scope through launch.** A trusted Antigravity launch into an existing directory uses the
  resolved directory shown on the card. Agent-requested fresh checkouts carry the Claude preparation choice from before
  approval through checkout and environment startup. An absolute Claude config override is required for automatic trust.
  A relative override produces the existing trust notice and the session continues.
- **Consent text is available to assistive technology.** Terminal hints are announced without moving focus. Settings
  exposes trust warnings and hook removal instructions. Approval cards expose the permission and trust text before the
  answer buttons. An unread workspace trust switch is labeled unavailable, then reports its saved value when read.
  Validation covered application metadata and adapter source. Screen-reader speech was outside the test coverage.
- **Hook options stay with the choice that opened them.** Returning to the same agent's tab checks an unanswered offer
  again. Choosing a model that switches the agent to Antigravity checks its offer too. A lost dismissal reply does not
  keep the hint hidden after a reconnect. A command launched with Resume off keeps its hook and diagnostic log without a
  user-facing missing-report warning about Restart.
- **Some fixes need a broader change.** Four optional polish fixes were left unchanged. The existing session reports
  cannot reliably distinguish a first observation after a restart from a brief recovery gap. Correctly retiring old hook
  errors or attributing a stale click also needs stronger ownership and lifecycle checks. The archive records the reason
  for each decision.
- **Changes across nearby plans stayed focused.** Launcher code changed only for the hook offer and Settings only for
  the trust and hook controls. Helm changes carry trust through existing approvals and checkout preparation. Supervisor
  changes are limited to the path check and the missing-report notification. Start a session and Uninstall Farhelm
  document the affected behavior. The rebase preserved main's launcher guard for the warning an answer actually showed,
  its approval target-connection guard, and its updated YOLO descriptions.

## Open questions and possible follow-ups

- **The warning clock can start during checkout preparation.** Pressing Enter during a slow clone or its setup command
  can start the countdown before the agent receives a prompt. Farhelm may then say Restart cannot resume because the
  hook has not reported. The follow-up preserves input during preparation and starts the countdown at the first
  submitted line the current agent can receive.
- **Hook setup failure text in Restart and recovery needs direction isolation.** A reason written right to left can
  reorder nearby words or punctuation in the explanation around it. The follow-up covers the session header's Restart
  button and menu and the interrupted-session card, with tests of the visible and accessible text.
- **Should an approval card show Claude trust when the requested session runs a different agent?** An agent-requested
  fresh checkout can mark its directory trusted in Claude's settings even when the session uses a different agent. If
  enabled, the edit persists for later Claude sessions in that directory, with best-effort removal when Farhelm archives
  the checkout. The setting now stays fixed from before approval. The current card can show the requested agent's trust
  choice without mentioning the separate Claude edit. Showing the Claude edit explicitly when it applies would tell the
  user the full effect. Keeping the card focused on the selected agent keeps it shorter but leaves this setting
  implicit. Show it conditionally if this is revisited.

## Pull requests

- Announced the trust and hook hints without moving terminal focus. Two fixed.
  [#4857](https://github.com/scode/farhelm/pull/4857/changes)
- Reset a local Hide and an unanswered hook dismissal when the desktop reconnects. Two fixed.
  [#4859](https://github.com/scode/farhelm/pull/4859/changes)
- Cleared a trust dismissal error after the saved dismissal. One fixed. Three unchanged within the polish gate: a hide
  surviving a same-session relaunch, a stale hook refusal, and a trust error from a stale click.
  [#4863](https://github.com/scode/farhelm/pull/4863/changes)
- Exposed workspace trust warnings and hook removal guidance in Settings under the desktop accessibility parity rule.
  Two fixed. [#4864](https://github.com/scode/farhelm/pull/4864/changes)
- Named the project settings and extensions Pi asks to approve. One fixed.
  [#4866](https://github.com/scode/farhelm/pull/4866/changes)
- Limited Claude's hint to a structured Claude launch or a session created in a fresh managed checkout. One fixed.
  [#4871](https://github.com/scode/farhelm/pull/4871/changes)
- Deferred the early hook warning during checkout preparation. Left the Antigravity dismissal during session recovery
  unchanged under the polish gate. [#4872](https://github.com/scode/farhelm/pull/4872/changes)
- Preserved unanswered setup offers across tab changes and checked the offer when a model pick changes the selected
  agent. Two fixed. [#4877](https://github.com/scode/farhelm/pull/4877/changes)
- Used the same resolved Antigravity directory on the approval card and in the launch. One fixed.
  [#4895](https://github.com/scode/farhelm/pull/4895/changes)
- Retained the Claude checkout trust choice from before approval during checkout and environment preparation. One fixed.
  [#4900](https://github.com/scode/farhelm/pull/4900/changes)
- Required an absolute Claude config path for automatic trust. One fixed.
  [#4902](https://github.com/scode/farhelm/pull/4902/changes)
- Exposed permission warnings before approval answers and labeled unread trust settings unavailable. Two fixed.
  [#4905](https://github.com/scode/farhelm/pull/4905/changes)
- Refused unsupported Grok and Antigravity hook setup paths and skipped the missing-report notice for launches with
  Resume off. Two fixed. [#4908](https://github.com/scode/farhelm/pull/4908/changes)
- Added the per-agent removal links to Uninstall Farhelm and deferred direction isolation in Restart and recovery. One
  fixed, one deferred. [#4911](https://github.com/scode/farhelm/pull/4911/changes)
- Extended the existing tests for independent Antigravity trust and a Claude session started in its home directory. One
  test outcome resolved. [#4914](https://github.com/scode/farhelm/pull/4914/changes)

## Checks

- At rebased tip `bd1abccd` on Linux, recorded
  `cargo nextest run -p farhelm-helm -p farhelm-supervisor -p farhelm-gui --lib` with a focused selection. All 418
  selected tests passed without runtime skips. They cover launcher offers and shown consent, approvals and trust
  settings, terminal hints, and supervisor hook setup and capture. The recorder ID is
  `6e58500f-447c-41a9-a75f-4b781ff2c5d7`.
- `python3 scripts/check-gui-goldens.py` matched twelve selected scenes in both appearances, covering launch and Restart
  approvals, the launcher hook question, terminal hints and agent settings. The recorder ID is
  `364c4bdb-ada8-4778-a51c-b4062ec83dc4`. `python -B scripts/check-test-sleeps.py` found zero delays without a rationale
  across 309 calls.
- Reused `bun install --frozen-lockfile` and `bun run build` in the website directory at `0767c69a`. The build validated
  both new uninstall link anchors. Later edits changed its follow-up entry and archive metadata. The rebase preserved
  the website patch, link destinations and build inputs. Astro reported its empty translation collection and missing 404
  entry warnings and completed.
- Reused the accepted per-draft source reviews and earlier visual reviews. Every rebased draft retains the same changed
  lines in code, tests, specifications, user docs and changelog. The rebase combined the shared archive, pending ledger,
  index and TODO entries. Later main changes assigned other TODO entries to plans.
- Skipped the full workspace, other platforms, and live vendor and installation suites. The focused tests exercise the
  changed contracts with isolated fixtures. The uninstall change adds guidance and link validation.

## Review gate

All fifteen units passed independent Sol high source review and the changed views received Opus high visual review in
both appearances. Source review tightened tab ownership, distinguished independent trust choices, and caught the Linux
off-state inference for an unread switch. The affected scenes matched again after the rebase. All 26 dispositions map to
the fifteen PRs above.
