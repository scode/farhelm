## What this was about

This delivery accounts for all 30 desktop review outcomes in ten open drafts awaiting the landing monitor. It covers
sessions and terminal input, the launcher, templates and approvals, and host and container settings. The decisions
called for narrow product fixes and cheap test improvements. The plan records the larger fixes that still need work and
the item already fixed on main.

## Things you should know

- A capped session list can omit a still-running shell without closing it. An already open shell tab now survives that
  omission. The separate case of an unavailable shell tab remembered from a previous opening remains a follow-up.
- During text composition, session Delete and Rename wait while the focused terminal still owns marked text. The quick
  launcher keeps incidental composed text from reaching the terminal behind it. The Wayland platform can commit text
  before delivering a physical key, so the first guard applies to marked text still present when the app dispatches the
  shortcut.
- Linux Ctrl+F could leave a focused Settings field at the end of the text. Focus now stays in the field. The clipboard
  file-list and reduced-motion findings need larger changes and have follow-up entries.
- Hosts' Suspend warning describes sessions and says their processes are preserved. Shell wrapper help in the launcher
  and Templates uses launch-command wording. The separate approval-card wording for an agent's Stop or Delete request
  remains open.
- Changing the launcher program clears a conversation ID from the old program. Enter on a Replace with folder suggestion
  selects the choice without submitting a session. Repository discovery refusals show the same escaped host text on
  screen and through accessibility. The shell program row was already grouped before Model by #4433. A stale launcher
  result that disappears without explanation remains a follow-up.
- Shell template approvals and the editor show the stored permission and flags. The editor marks incompatible fields and
  still refuses Save until the incompatibility is removed. Long summaries use the existing list ellipsis, with full
  values visible in the editor. Draft prompts now survive a template switch to a program that cannot accept one and
  return when an eligible program is selected. The shared launch resolution sends the prompt only to a compatible final
  program, including when a CLI launch uses the template.
- Saving an untouched alias closes its editor without sending a request that could undo a concurrent rename. Desktop
  command Debug output keeps routing context and omits request bodies that could carry credentials.
- Hosts withholds lifecycle actions while a stopped container waits for its container host. Settings → Containers keeps
  the earlier leftovers alongside a failed refresh. Discovery still succeeds or fails as a whole. A successful Delete
  removes that row while the follow-up listing is pending. The next successful listing is what can establish that the
  list is empty.
- The test-gap changes use existing fixtures. The password test checks its role and accessible name as well as the input
  value in the model and its masked paint. The broader per-form, accessible-value and log-capture work fell outside the
  approved small test changes.

## Open questions and possible follow-ups

Five larger changes are tracked in Near term.

- **Show the main terminal when a remembered shell tab cannot attach.** The terminal can stay on the saved shell tab and
  keep reconnecting. The available failure does not distinguish a permanently missing shell from a transport refusal.
  Clearing the tab on both failures can discard recoverable state and its notices.
- **Attach files pasted from a Wayland file manager.** The file-list work must also preserve non-file links in the same
  clipboard offer. That takes a fallback that reads the other representation while the offer is still live.
- **Use the desktop's reduced-motion setting for the attention cue and other motion.** The cue needs a visible resting
  state, and its animation needs to resume when reduced motion is switched off. The current toolkit path needs wider
  work to preserve both.
- **Name the program a Stop or Delete approval would end.** The approval needs to carry the target's launch program to
  name it reliably. It must keep the existing target checks and handle older stored approvals.

- **Explain stale launcher choices.** Pressing Enter on a recent setup that just disappeared can do nothing. Clicking a
  search result for a vanished host can also clear the search without selecting anything. The tested warning needed
  substantially more code and screenshot work than the triage budget allowed.

The launcher outcomes overlap earlier launcher and hook setup plans. The relevant groundwork was already on main before
this plan took the work. The template prompt fix crosses into the helm's launch resolution so CLI shell launches still
work. The container work overlaps the product area of Podman and environment plans, and changes desktop presentation
without altering discovery or provisioning. The terminal work shares the desktop terminal with the exited-scrollback
plan and affects tabs, input ownership and a link test.

## PRs

- [#4526](https://github.com/scode/farhelm/pull/4526/changes) Fixed live tabs missing from a capped list. Deferred the
  remembered-tab recovery.
- [#4528](https://github.com/scode/farhelm/pull/4528/changes) Fixed both composition behaviors, for session Delete or
  Rename and for quick launch.
- [#4531](https://github.com/scode/farhelm/pull/4531/changes) Fixed Ctrl+F in Linux Settings. Deferred Wayland file
  paste and reduced motion.
- [#4537](https://github.com/scode/farhelm/pull/4537/changes) Hosts Suspend describes the affected sessions, both
  wrapper editors describe the launch command, and approvals tests check the displayed shell and full command. Deferred
  Stop or Delete approval wording.
- [#4545](https://github.com/scode/farhelm/pull/4545/changes) Fixed the conversation selector and Replace with Enter
  behavior. Discarded the shell-row item already fixed by #4433.
- [#4548](https://github.com/scode/farhelm/pull/4548/changes) Fixed repository discovery status text. Deferred stale
  launcher warnings.
- [#4553](https://github.com/scode/farhelm/pull/4553/changes) Fixed all three template outcomes, covering the approval
  card, incompatible editor values and dormant prompts.
- [#4556](https://github.com/scode/farhelm/pull/4556/changes) Saving an untouched alias preserves a concurrent rename.
  Desktop request diagnostics omit bodies that can carry credentials.
- [#4563](https://github.com/scode/farhelm/pull/4563/changes) Fixed the stopped-container menu and leftover-list refresh
  as decided.
- [#4565](https://github.com/scode/farhelm/pull/4565/changes) Fixed the cheap tests for Reset focus, restart failure,
  terminal input replay counts, the second wrapped-link open, sound switches following saved values and SSH answer
  addressing. Narrowed the secrets item to the password field's accessible names.

## Checks

The stack was rebased onto `288385def0a7`. Main gained post-exit terminal browsing and selection, checkout-home fallback
for disposable environments, a recipe-delete confirmation marker and screenshots, a status-mark hover change, and
related supervisor and helm work. The launcher keeps both the missing-folder suppression for a new environment and the
escaped text on a refused repository request. Terminal composition and the wrapped-link test coexist with local pointer
handling and navigation after exit. The template resolver retains main's placement documentation, and command Debug and
the recipe-delete marker edit different parts of the protocol types. The combined screenshot manifest preserves both
sets of changes.

- Ran a nine-case `cargo nextest run -p farhelm-gui --lib` selection on the rebased tip `ef512e9dd48f`. The
  carried-session, composed-input, wrapped-link, post-exit scroll and pointer, restart-help, and both checkout cases
  passed (recorder `457ce5cf-8115-42e7-9ca7-e58a1ee4a63d`).
- Ran the full `python3 scripts/check-gui-goldens.py --jobs 4 -j 4` on that tip. All 386 picture references matched
  (recorder `768e6c70-320e-4427-b8e7-58364c420e8a`).
- Reused the focused `cargo nextest run -p farhelm-gui --lib` tests below. The revisions identify the original drafts
  carrying the tested behavior. Subsequent changes before the rebase were presentation fixtures or outcome records. The
  rebase retained these paths and directly rechecked the behaviors that interact with main's changes.
  - Tab omission and ordinary tab behavior at #4526 (`f159a381bf18`, recorder `52fc8e64-6411-4f88-8dc6-9ed287ebb82d`).
  - Session shortcuts and quick-launch input at #4528 (`e4c3092b2ff6`, `111098c7-9c88-4301-b880-1ae38710f5b8`).
  - Linux Settings Ctrl+F at #4531 (`8d070babfde4`, `25ec9c78-ba6c-4484-9939-7bed25dd844c`).
  - Shell approval facts and Suspend wording at #4537 (`a7e28ebf0dda`, `10fe32b6-75ff-47d7-9f61-0df619b8c777`), and the
    two unchanged wrapper validation cases from `b3a08c69-05b2-4b78-95e8-2226c2680799`. That earlier run's shell
    approval fixture failed and was corrected in the first listed run.
  - Launcher conversation and Enter decisions at #4545 (`5811208bcfab`, `a78792e9-75fc-4983-9853-0ba668b99cba` and
    `83b82bc5-9533-4d9e-960c-1571e8e54311`). Repository refusal was rerun on the rebased tip.
  - Template permission and prompt behavior at #4553 (`3713ad1362a5`, `625d612f-0654-4606-84cc-7014f576f104` and the
    unchanged GUI approval and helm resolver cases in `e8562a9b-2f89-481d-97d5-d304905fb2d8`, which also selected
    `-p farhelm-helm`).
  - Host alias and command Debug at #4556 (`b6d0946ed73e`, `f69315c6-7747-48a8-a21a-774611f91735`, which also selected
    `-p farhelm-proto`).
  - Container menu offers, refresh recovery and Delete at #4563 (`14bcb7fe789c`,
    `5c0cd34e-2e84-4f5d-a12e-1cbf7560ba4d`).
  - Desktop test-only checks at #4565 (`634040ca3e52`, `89ec9a4b-7abd-418c-b54a-09a76d7b6b22` with two cases superseded
    by `5f57db86-f3d1-4ec2-a3cb-b81603efe8cd`). Wrapped-link and restart cases were rerun after rebase. The two
    `cargo nextest run -p farhelm-input-mapping --lib` replay tests passed (`6c3146c0-a637-4df0-b951-6262b5296f73`),
    including 1,730 keyboard and 768 mouse fixture rows.
- Reused the 47-case `python3 -B scripts/test-gui-screenshot-batch.py` suite from #4563 (`14bcb7fe789c`,
  `e43720ed-f12f-462b-86f2-e0aeae412474`). The catalog harness and this plan's registrations carried forward, and the
  combined comparison verified the rebased manifest.
- `git diff --check`, scoped `dprint`, `python3 releasing/check-changelog.py format` and
  `python -B scripts/check-test-sleeps.py` passed. `python3 scripts/check-agents-md.py` found zero errors and five
  existing citation warnings at the rebased revision. They concern one archive heading, a missing plan-report link and
  three deflake heading references.
- Skipped the full workspace test battery and hosted CI. The changes that interact with main were exercised through the
  focused local runs and screenshot comparison.

## Review gate

Each draft passed independent source review before rebasing. The rebase reconciliation was checked against the original
patches and the combined behavior was verified by the focused tests and screenshot comparison. Changed rendering passed
independent Opus visual review in both appearances, and the reviewed changed pictures were published with their
manifests. The existing one-line truncation of long template summaries remains. For the container leftovers scene, the
discovery reason stays in the list above the open Delete question. The interaction tests independently exercised the
failed read and successful recovery.
