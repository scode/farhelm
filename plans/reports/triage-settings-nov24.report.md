## What this was about

This plan resolved all 33 triaged Settings items across 20 PRs. The verified draft stack is ready for the landing
monitor. It fixes cases where Save and Close could save an edit you had never approved, miss a conflicting Tensorlake
key or dismiss a newer question. It also improves save refusal messages, keyboard and screen-reader behavior, and tests
for what happens to credentials on a failed write and retry. There are 21 fixed outcomes, nine deferred outcomes and
three closed with no change under the agreed small-fix limits.

## Things you should know

Save and Close now binds to the drafts shown when the close question opens. A later edit or a Codex login file that
finishes loading while the question is open needs its own Save. Settings stays open when a newer question still needs an
answer. If a test or key action currently holds Save, the footer explains why you need to retry. A failed Notifications
save brings you back to the failing control. An open custom-agent Delete confirmation explains why it blocks Save, and
an in-flight Save keeps keyboard focus through catalog updates.

The trust help for Codex, Muse, Pi and Antigravity now covers launches an agent requests, including a folder it chooses
for a request Farhelm approves without asking. It describes the existing rules for installed hosts, a session's own
sandbox or container, and reusing a saved command. Search options announce the built-in or registered agent's display
name. Registered agents with identical display names follow the existing navigation naming convention.

The custom-agent fixes fit the registry, editor and Search changes from the November 24 custom-agent plan. The
Tensorlake discovery follow-up already exists from the Environments plan, and the saved-key conflict fix preserves its
separate cleanup protections. The trust text incorporates the workspace-trust launch policy from the post-clone Claude
trust plan. Hook and tunnel changes here govern what Settings presents and accepts, while the Hooks and Trust, Hosts and
Tailscale plans govern launch hooks or host transport. The credential-delivery change checks the actual files given to a
launch after retry. It adds test coverage for the Helm and provisioning work.

## Open questions and possible follow-ups

Seven new TODO entries cover eight deferred outcomes. The stale-footer fix is important because navigation during a
press can make Save replace a stored Tensorlake key, and Discard can throw away a refused draft on a different page. A
failed hook opt-out also needs to stay visible after Settings closes, because later launches can still install hooks.

The remaining close fixes stop the batch when an independent save fails, reveal the hidden credential editor after a
refusal, wait for a credential write already sent before Back or Cancel, keep credential delivery warnings visible and
name each affected custom-agent draft. The hidden editor, credential warning and multi-draft prototypes crossed the
agreed complexity limit once they handled the close paths, accessibility and regression tests. The existing Tensorlake
TODO covers the ninth deferral. It needs to prevent actions from an old key's discovery results during a key change and
after an ambiguous response.

Three items were closed under the agreed cheap-fix limits. Exercising masked-value accessibility in the test platform
required a new dependency path. Keeping cached Containers rows and a loading state during retry needed to coordinate
both Refresh and the read after Delete. The bare or lingering Tailscale refusal needed to coordinate concurrent launch
and toggle notices with the right switch. Each disposition and reason is in the archive.

## Pull requests

- Deferred both stale-footer cases to one follow-up, covering Save targeting the wrong page and Discard targeting the
  wrong draft after a refusal. [#4960](https://github.com/scode/farhelm/pull/4960/changes)
- Fixed detection of conflicting Tensorlake credentials when closing with drafts in both key editors.
  [#4963](https://github.com/scode/farhelm/pull/4963/changes)
- Fixed three approval cases. Save and Close uses the displayed drafts, a queued save uses its accepted edit, and a
  Codex login file staged later needs its own Save. [#4966](https://github.com/scode/farhelm/pull/4966/changes)
- Deferred stopping Save and Close when an independent write fails while the close flow is waiting.
  [#4968](https://github.com/scode/farhelm/pull/4968/changes)
- Fixed Notifications so a refused Save from the close flow reveals its error and the affected form.
  [#4973](https://github.com/scode/farhelm/pull/4973/changes)
- Deferred revealing the credential editor when a failed write originated from its hidden draft.
  [#4976](https://github.com/scode/farhelm/pull/4976/changes)
- Deferred showing a failed hook opt-out after Settings closes.
  [#4979](https://github.com/scode/farhelm/pull/4979/changes)
- Fixed the missing explanation when a test or another form operation is holding Save.
  [#4982](https://github.com/scode/farhelm/pull/4982/changes)
- Fixed dismissal of a newer question when a previously approved save completes.
  [#4986](https://github.com/scode/farhelm/pull/4986/changes)
- Deferred waiting for an independent credential write after the editor's Back or Cancel.
  [#4987](https://github.com/scode/farhelm/pull/4987/changes)
- Deferred keeping a saved credential's delivery report visible through Save and Close.
  [#4990](https://github.com/scode/farhelm/pull/4990/changes)
- Fixed the missing Save refusal while a custom-agent Delete confirmation is open.
  [#4995](https://github.com/scode/farhelm/pull/4995/changes)
- Fixed the custom-agent Save button losing focus or its draft when the catalog updates before the save reply.
  [#5000](https://github.com/scode/farhelm/pull/5000/changes)
- Deferred naming each affected custom-agent draft in the close question.
  [#5006](https://github.com/scode/farhelm/pull/5006/changes)
- Fixed keyboard activation of a hook switch using a value that had yet to be displayed. Closed the masked-input test
  gap with no change under the test size limit. [#5007](https://github.com/scode/farhelm/pull/5007/changes)
- Fixed the four agents' trust help and the accessible names of agent Search options.
  [#5010](https://github.com/scode/farhelm/pull/5010/changes)
- Deferred the stale Tensorlake sandbox list after a replacement key to the existing saved-key consent TODO.
  [#5013](https://github.com/scode/farhelm/pull/5013/changes)
- Closed six test gaps covering Pi's trust endpoint, late trust replies, Search focus, all four sound previews,
  Tensorlake Discard, and a busy Notifications test. [#5015](https://github.com/scode/farhelm/pull/5015/changes)
- Fixed App Settings naming the local and shared scopes and the sound Play hitbox intercepting clicks outside the
  button. [#5016](https://github.com/scode/farhelm/pull/5016/changes)
- Fixed the credential retry test to check the managed launch file, the preserved Codex editor setting and both
  credentials after retry. Closed the Containers refresh and Tailscale refusal items with no change under the small-fix
  limit. [#5019](https://github.com/scode/farhelm/pull/5019/changes)

## Checks

- Ran
  `cargo nextest run --locked -p farhelm-gui -p farhelm-helm --lib -E 'test(/^settings::/) | test(backend_failures_leave_saved_values_and_safe_warnings)'`
  through the recorder. It selected 244 Settings and mocked credential delivery tests. All passed. Recorder run
  `3a6c453f-fe5e-4c5a-bad2-299946af9737` on `ceeeec6a2`. The final draft tip `bbc6a2260` differs only in the published
  reference hashes.
- Ran `python3 scripts/check-gui-goldens.py --update --jobs 4` through the recorder for all 552 pictures. Its only eight
  changes matched the reviewed trust pictures byte for byte. The other approved Settings pictures stayed identical.
  Recorder run `cb6c40ed-ecd5-454f-93a8-c72f46c7afa1` on `ceeeec6a2`.
- Ran `cargo fmt --all -- --check`, `python3 releasing/check-changelog.py format`,
  `python -B scripts/check-test-sleeps.py` with the isolated checker, `git diff --check` and
  `python3 scripts/check-agents-md.py`. The instruction link check reported one inherited archive heading warning and
  zero errors. Checked all 20 draft heads and bases after the push. The stack is based on `6067673cc`. Later main
  changes to Welcome and the plan queue affect separate flows and golden scenes.
- Reused the custom-agent Delete and busy Save image approvals on `d97d04da7` and `7e7eb6615` and the App approval on
  `975f329ce`. The final render matched their published references. The final focused selection reran the interactions
  and the credential retry witness. Skipped unrelated workspace suites and real provider or installer tests because this
  stack changes Settings presentation and form behavior and the backend change is test-only. Skipped docs capture
  because no current shot uses the altered Settings views.

## Review gate

The review gate passed. Sol high reviewed each draft and the affected rebase code. Opus high reviewed each drawing
change in both appearances and approved the final rebased trust pictures. Independent scope reviews confirmed the
recorded deferrals where a smaller complete change could not be justified. Opus noted that Antigravity's longer trust
card leaves the last hook description line below the initial viewport. It is still available by scrolling, and the
entire trust card fits in both appearances.

### Landing

#### What landed with it

Only this plan landed this round, through its 20 PRs and the landing fix
[#5026](https://github.com/scode/farhelm/pull/5026/changes). Since the PRs were based, main gained the Welcome
empty-pane change, the hook log acknowledgment and screen capture tooling changes, a planning PR and a TODO entry. The
rebase applied without conflicts, and the landing's independent review found no interaction: nothing in this plan
assumes the removed startup Welcome card, its new screenshots sit beside the Welcome ones in the picture list, and the
Settings tests run on a view of their own, so the Welcome change's new focus home does not reach them. The review found
no path where Save and Close loses an edit, saves a draft you did not approve, or hides a refusal, and every executed
outcome left the queue and the triage ledger as the rules require.

#### Fixes made while landing

- **Six new Settings screenshots were missing from the screenshot scene list,** so the batch tool's own test failed on
  the stack (it requires every picture to be listed). [#5026](https://github.com/scode/farhelm/pull/5026/changes) lists
  them.
- **Removing triaged items left a doubled blank line in the review feedback queue's index,** which `dprint check`
  refuses. [#5026](https://github.com/scode/farhelm/pull/5026/changes) removes it.
- **Three deferred TODO entries describe more than triage decided (the review's should-fix).** The entries for a failed
  hook opt-out after Settings closes, naming each custom-agent draft when closing, and keeping credential delivery
  reports visible each spell out a larger fix than the triage decision and read as settled; for the custom-agent and
  credential delivery ones, which triage sized as small, that larger scope is itself the stated reason for deferring.
  [#5026](https://github.com/scode/farhelm/pull/5026/changes) adds to each what triage actually decided and marks the
  rest as the executor's proposal, so a later executor does not take it as your design.

#### Checks

Worker runs were on a separate arm64 Linux test worker; the rest ran locally on x86_64 Linux. All the checks below ran
on the plan's 20 PRs rebased onto main as it stood at the start of the landing (`a1090987`), which applied without
conflicts; every file's change is exactly what the executor pushed. The landing fix
[#5026](https://github.com/scode/farhelm/pull/5026/changes) changes only the scene list, the queue index's whitespace
and three TODO entries, so these runs still apply to it.

- The GUI and helm crates' tests, `cargo nextest run -p farhelm-gui -p farhelm-helm`, on the test worker (recorder run
  `5731c272-82be-4e46-8c51-3c607520afe0`): all 5365 passed. Only these crates' code changed.
- `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings`,
  `cargo clippy -p farhelm-desktop --all-targets -- -D warnings` and
  `cargo clippy -p farhelm-desktop --all-targets --features agent -- -D warnings`, on the test worker: passed.
- The desktop app's end-to-end smoke, `python3 -B scripts/gui-e2e-smoke.py`, on the test worker (recorder run
  `0e0176c5-28d3-4b35-aca6-17cac6f3d140`): of its 49 steps, 48 passed and one was skipped by design on Linux (paste),
  including opening and closing Settings.
- The desktop app's screenshot comparison, `python3 scripts/check-gui-goldens.py`, all scenes, locally (recorder run
  `6d0c1c64-5c20-41e0-8e74-3edd23c985c3`): passed on the rebased stack, including the plan's new and changed Settings
  pictures and the Welcome pictures that landed since its base.
- The screenshot batch tool's test, `python3 -B scripts/test-gui-screenshot-batch.py`, locally: failed on the stack
  (recorder run `64a31d1a-c5e4-4a65-8bdf-5f0e62e5d042`) because six new pictures were not in the scene list, and passed
  after [#5026](https://github.com/scode/farhelm/pull/5026/changes) (recorder run
  `828907ec-9623-4cf9-8c22-86cee2e2e61f`).
- `python3 scripts/check-agents-md.py`, `python3 releasing/check-changelog.py format`, `cargo fmt --all -- --check` and
  `python -B scripts/check-test-sleeps.py`, locally: passed. `dprint check`, locally: on the stack it also failed on the
  review feedback queue's index, which [#5026](https://github.com/scode/farhelm/pull/5026/changes) fixes; after that it
  fails only on over-long lines in the triage outcomes archive, which main has had since an earlier triage record
  (#4978) landed. No PR fixes them yet; a formatting-only one would, and I recommend it, as earlier rounds' notes did.

Reused across [#5026](https://github.com/scode/farhelm/pull/5026/changes): every run above except the scene list test
and `dprint check`, which ran again. The executor's runs were on an older base, and everything they covered ran again
above.

Skipped: the rest of the workspace Rust suite and the desktop app's own tests, since no other crate's code changed; a
Mac run, since nothing changed is Mac-specific; and CI, which runs only on demand.

#### What the report says that is no longer true

- Nothing.

#### Things you should know that the report does not say

- **Nine of the 33 outcomes were deferred, and two that triage sized as small were deferred on scope the executor added
  itself** (the custom-agent draft names and the credential delivery reports, the two described above; the review did
  not single out the others). The TODO entries now say which parts were decided; you may want a smaller fix for those
  two than the entries propose.
- **The new trust help partly does an outcome of a queued plan.** Its wording for launches an agent requests already
  covers some of what `triage-trust-settings-and-ui-wording`, which runs after this one, is to change in the same help
  text; its executor should check before repeating it.
- **Smaller points the review noticed.** When Save and Close skips a draft that changed after the question opened,
  Settings stays open without saying why (no edit is lost, and the spec allows it); the changelog draft for keeping
  newer questions open does not say Settings has to be closed again afterwards; the new gate on the hook switch's
  displayed state covers only that switch, not the workspace trust switches, which are also consent; and a few wording
  and quoting details in the new TODO entries and one refusal message. None needs a follow-up on its own.
