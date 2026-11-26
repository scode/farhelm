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
