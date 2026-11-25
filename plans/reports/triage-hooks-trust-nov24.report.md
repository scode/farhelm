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

### Landing

#### What landed with it

Only this plan landed this round, through its fifteen PRs and the landing fix
[#4926](https://github.com/scode/farhelm/pull/4926/changes). Since the plan's PRs were written, main gained only plans
queue commits (a planning PR and other plans' deliveries), so nothing could interact with it.

The landing's review read the whole diff against SPEC.md and SPEC_impl.md and found no bug in the code and nothing that
widens automatic workspace trust. "Claude's trust switch" below is the Settings switch under Agents → Claude Code that
answers Claude's "trust this folder?" question automatically. A fresh checkout an agent asks for now reads that switch
once, when its approval card appears, and both ways such a checkout is prepared (on an existing host or in a new
environment) use that reading, so a Settings change while the card waits cannot change what Allow does. For a trusted
Antigravity launch, the folder the launch trusts is now settled before the card appears, and it is the folder the card
shows. A relative Claude config directory now skips automatic trust with a note in the terminal. Both of those only
narrow trust. No database schema or protocol version changed.

The review counted no blockers, one should-fix and four small points. The should-fix is the first landing fix below.
Three of the small points are the first three items in the last section of these notes. The fourth needed no change: the
list of agents whose approved folder is settled before the card now names exactly the agents that have a workspace trust
switch, and the repository keeps such per-agent lists separate on purpose even when they match.

#### Fixes made while landing

- **SPEC.md said agents could no longer use Claude's trust switch (the review's should-fix).** The plan narrowed where
  Claude's trust hint appears, and on the way changed the sentence on which launches read Claude's trust switch from
  "agent-requested launches" to "Spawn" (an agent starting a child session). Farhelm still applies the switch when an
  agent asks to create or clone a Claude session, as SPEC.md's paragraph on the Codex, Muse and Pi trust switches and
  the Start a session docs page say, so the spec disagreed with the code. #4926 puts the old wording back.
- **Unwrapped entries in the triage archive (found by the Markdown formatter's check, not the review).** Several of the
  archive entries the plan's PRs added were not wrapped, so the formatter's check failed on the plan's final revision.
  #4926 rewraps them and changes no words.

#### Checks

Worker runs were on a separate arm64 Linux test worker; the rest ran locally on x86_64 Linux. All the checks below ran
on the plan's PRs rebased onto main as it stood at the start of the landing (`bb92d1b0`), without conflicts; the rebased
stack's top commit was `9e67a5f3`. The only later change is the landing fix
[#4926](https://github.com/scode/farhelm/pull/4926/changes), which changes one SPEC.md sentence and rewraps the triage
archive; after it I ran `dprint check` and `python3 scripts/check-agents-md.py` again, and both passed. That final tree
is what main now holds outside the plans queue.

- The workspace Rust suite, `cargo nextest run --workspace --exclude farhelm-desktop`, on the test worker (recorder run
  `70081dc6-69dc-47b5-ba47-6afd9d1b1aab`): all 8661 tests passed, with no runtime skip. The plan changed the helm's
  agent approvals, sessions and launches, how the supervisor applies Claude's trust switch at launch, its hook setup and
  its missing-report notice, and the GUI's session pane, launcher, approval cards and Settings, with their tests; the
  whole suite was the narrowest run that covered them and their callers.
- The desktop app's tests, `cargo nextest run -p farhelm-desktop`, on the test worker (recorder run
  `a8a24543-9539-49d6-be97-1116728d0117`): all 184 passed. The desktop app builds the GUI crate this plan changed.
- `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings`,
  `cargo clippy -p farhelm-desktop --all-targets -- -D warnings` and
  `cargo clippy -p farhelm-desktop --all-targets --features agent -- -D warnings`, on the test worker: passed.
- The GPUI end-to-end smoke, `python3 -B scripts/gui-e2e-smoke.py`, on the test worker (recorder run
  `1f587b07-e12e-4063-ac8d-4bd27f96ec14`): 50 steps passed, one skipped by design on Linux (paste). It drives the
  launcher, Settings, approval cards and terminal attach, which the plan changed.
- The GPUI app goldens, `python3 scripts/check-gui-goldens.py`, all scenes, locally (recorder run
  `92057fa9-1fd8-453d-9af8-b7b09bd8c247`): passed, including the two Settings scenes whose references the plan updated.
- The docs website build, `bun install --frozen-lockfile && bun run build` in `website/`, locally: passed. The plan
  changed two docs pages, Start a session and Uninstall Farhelm.
- `cargo fmt --all -- --check`, `python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py` (no
  errors; one warning, about an old citation in the triage archive, already on main) and
  `python -B scripts/check-test-sleeps.py`, locally: passed. `dprint check`, locally: failed on the triage archive, as
  above, and passed with the landing fix.

Reused: none. The executor's own runs were focused selections before the rebase, and everything they covered ran again
above.

Skipped: the test of the screenshot comparison tool's list of GUI scenes, since the plan changed two golden references
but added or removed no scene; the workspace doctests, since no documentation example changed; a Mac run, though the
screen-reader changes also apply on macOS (see the last item in the last section of these notes); and CI, which runs
only on demand.

#### What the report says that is no longer true

- Nothing. The report does not claim the Markdown formatter's check passed, and every claim in it matches the diff.

#### Things you should know that the report does not say

- **One test covers only half of the approval rule it was written for.** The test that checks an approved fresh checkout
  uses Claude's trust switch as it was when the card appeared covers only the switch being turned off while the card
  waits. The triage called the other direction the more serious case: the card says the checkout will not be trusted,
  the user turns the switch on in Settings meanwhile, and the checkout gets trusted anyway. The code handles both
  directions the same way, so it is correct today, but nothing would catch a change that broke only that direction. I
  recommend adding that case to the same test; say if you want a TODO entry.
- **The hook setup switch on the Grok and Antigravity Settings pages still tells screen readers "off" before its value
  has loaded.** The plan fixed exactly that for the workspace trust switches, which now read as unavailable until their
  value is known; the hook setup switch was not among the plan's findings. A screen-reader user whose hook setting is
  slow to load or fails to load hears "off" even when hooks are on. The fix is one line using the same unavailable
  state; say if you want a TODO entry.
- **The new rule that consent, warning and status text must reach assistive technology sits under SPEC.md's "Keyboard
  focus" heading.** That section's note says it is the rulebook for where focus moves, so a reader looking for
  accessibility rules would not look there. SPEC.md has no accessibility section, so moving it means adding one (an
  "Accessibility" heading in the desktop app's part of the spec, say). I left that spec edit to you.
- **Nothing has checked what a screen reader actually says.** The tests check the accessibility information the app
  exposes on Linux, not what VoiceOver or Orca speak. The plan exposes an unread switch as an unavailable label because
  some platforms' accessibility layers read a switch with no value as off, and nothing has confirmed the macOS result. A
  devbuild on the Mac with VoiceOver would; say if you want one.
