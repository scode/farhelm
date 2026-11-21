## What this was about

This plan delivered four drafts to the landing monitor. Triage asked for fixes grouped by area, plain shell wording in
shell sessions, and narrow tests for user-facing behavior. This stack covers the session list, environment actions,
attention and notices. The plan's complexity limit sends three larger product fixes to TODO. The test-fix limit leaves
one test gap open.

## Things you should know

- Screen readers lost a session's group context when the header scrolled away. The member row now announces its
  displayed group. A Shift-drag release beyond the rows could leave sidebar order frozen. When the window receives the
  release, it now applies the waiting order.
- Starting a retry could hide a prior Update or setup failure behind the new progress bar. The amber warning now stays
  visible alongside progress until the failure is acknowledged or resolved. A separate documentation correction says
  that an upload result can reappear on the next shown terminal, including a shell tab.
- While a sandbox or container was being deleted, its session card could promise that the session kept running and would
  reconnect. The card now says the environment is being deleted and its actions are unavailable. The suspended card used
  to mention the agent's command even for a shell session. It now says the session's processes continue on Resume.
- A failed refresh kept the previous session list. If that cache looked idle, Suspend could go ahead while current
  activity was unknown. A failed refresh now requires confirmation.
- Stop's confirmation could ask to close the remaining tabs while its button help still described stopping the main
  program. The Confirm and Cancel help now describes the tabs. Delete in a shared environment used to explain only the
  scope of whole-environment deletion. It now also names the live program and tabs it stops in the selected session,
  even if the environment stays available to other sessions.
- GUI coverage now follows the actual focus in Trash and SSH sign-in, observes the Delete command at the confirmation
  boundary, and checks a reduced-motion cue in the frame after window activation. It also covers the complete clickable
  documentation address in a notice and the shell wording in both Stop and suspend paths.
- The spec-text plan clarified Suspend evidence, leftover background jobs and sign-in visibility. The wording here stays
  within those contracts. The Tensorlake and Podman triage plans cover the provider lifecycle behind the shared
  environment views. The agent-hook plan covers agent setup and Restart wording. This stack changes session controls and
  card copy within those shared areas.

## Open questions and possible follow-ups

Three deferred outcomes are in TODO.md under Near term → Plan follow-up.

- **Keep bulk Stop and Delete bound to what the user saw.** A marked row can still exist after its host changes, or
  after another session joins its environment. The operation needs to preserve the drawn confirmation and verify that
  its scope still applies both at the click and before sending each queued request.
- **Pace metadata changes to marked sessions.** Sending more commands than the helm can keep pending can reconnect the
  window with a partly applied batch. A reply-paced queue that preserves per-row writes and busy-row behavior needs its
  own change.
- **Keep interrupted uploads visible after their session goes away.** A successful Delete or Replace behind a filter can
  end the session without delivering a complete list that proves it gone. Upload results need a recipient that survives
  the last terminal closing and also covers that filtered case.
- The below-fold SSH sign-in test sees the settled frame. It could pass if the popover opened before the sidebar scroll
  revealed its anchor, briefly leaving the question without a visible host control. Proving the order needs a
  first-frame test witness beyond the plan's narrow test work.
- In the tabs-only Stop prompt, the third button's Stop and suspend explanation can still say the shell will stop after
  the shell has already exited. The test covers the inline question for a running shell and its tabs. The tabs-only
  wording fix covers the Confirm and Cancel buttons.

## Pull requests

- Session group descriptions, Shift-drag release and the two bulk-action deferrals.
  [#4469](https://github.com/scode/farhelm/pull/4469/changes)
- Warning edge during a retry, retirement documentation and the interrupted-upload deferral.
  [#4477](https://github.com/scode/farhelm/pull/4477/changes)
- Unavailable session cards and the Suspend, Stop and Delete questions.
  [#4481](https://github.com/scode/farhelm/pull/4481/changes)
- GUI test coverage for session actions, Trash focus, SSH sign-in, notices and attention cues.
  [#4486](https://github.com/scode/farhelm/pull/4486/changes)

### Outcome dispositions

| Outcome                                                                 | What happened                                                                                                  | PR    |
| ----------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------- | ----- |
| `bulk-action-uses-live-shell-guard.md`                                  | Deferred to “Keep bulk Stop and Delete bound to what the user saw.”                                            | #4469 |
| `bulk-mute-can-reset-window.md`                                         | Deferred to “Pace metadata changes to marked sessions.”                                                        | #4469 |
| `session-groups-lose-membership-in-accessibility-tree.md`               | Fixed. A member row supplies its displayed group in its accessible description.                                | #4469 |
| `sidebar-order-stays-held-after-shift-sweep.md`                         | Fixed. A release the window receives outside the rows ends the order hold.                                     | #4469 |
| `working-attention-row-hides-warning-edge.md`                           | Fixed. The edge stays visible while the retry draws progress.                                                  | #4477 |
| `retired-terminal-comment-names-agent-recipient.md`                     | Fixed. The documentation refers to the next shown terminal.                                                    | #4477 |
| `interrupted-upload-notice-lost-on-session-removal.md`                  | Deferred to “Keep interrupted uploads visible after their session goes away.”                                  | #4477 |
| `trailing-ui-deleting-environment-retains-running-reconnect-promise.md` | Fixed. The card describes deletion and unavailable actions.                                                    | #4481 |
| `suspended-shell-card-names-agent.md`                                   | Fixed. The card describes resuming the session's processes.                                                    | #4481 |
| `suspend-treats-failed-session-refresh-as-complete.md`                  | Fixed. Failed refresh requires confirmation.                                                                   | #4481 |
| `shell-stop-tab-only-help.md`                                           | Fixed. Confirm and Cancel name the remaining tabs.                                                             | #4481 |
| `shared-environment-delete-omits-live-shell.md`                         | Fixed. Delete names the live program and tab effects in the selected session.                                  | #4481 |
| `notice-docs-link-test-accepts-plain-text.md`                           | Fixed. The test requires a clickable link with the complete address.                                           | #4486 |
| `dialog-scroll-focus-loses-regression-check.md`                         | Fixed. The Trash test follows focus when the last scroll stop disappears and on the next Tab.                  | #4486 |
| `attention-cue-tests-miss-witnesses.md`                                 | Fixed. The tests observe stationary paint after activation and verify a fresh clipped request is refused.      | #4486 |
| `attention-baseline-test-never-cues.md`                                 | Fixed. The first snapshot can produce a cue later, and the retry uses the real sign-in state.                  | #4486 |
| `sign-in-notice-test-omits-unrelated-focus-case.md`                     | Fixed. A new attempt stays in the sidebar when the touch notice completes.                                     | #4486 |
| `sign-in-reveal-test-misses-first-frame.md`                             | No change. Observing the first frame needs additional test support.                                            | #4486 |
| `sign-in-focus-test-accepts-either-host.md`                             | Fixed. The unique prompt identifies the focused host while Retry is pending.                                   | #4486 |
| `sign-in-handoff-test-uses-connected-fixtures.md`                       | Fixed. Both hosts enter the fixture waiting for sign-in.                                                       | #4486 |
| `delete-shortcut-test-misses-early-request.md`                          | Fixed. The test observes zero Deletes before confirmation and one after.                                       | #4486 |
| `shell-stop-and-suspend-wording-untested.md`                            | Fixed. Tests assert shell wording for the direct action and for the inline question drawn for a running shell. | #4486 |

## Checks

- For delivery, Markdown formatting and `git diff --check` passed. Each rebased draft's source, test fixtures, golden
  manifest and archive additions were compared against the original drafts. Earlier Rust formatting, test delay
  rationale and changelog fragment format checks passed. The instruction checker passed with its existing
  archive-heading warning.
- Reused the focused runs and visual review after rebasing. Main's changes in this interval affected queue records,
  specifications and source comments. The Rust patches, test fixtures and golden manifest remained byte-for-byte the
  same across the rebase. The pinned `cargo nextest run --locked -p farhelm-gui --lib -E …` runs were recorded on Linux.
  - #4469, content in `9b4d44361c4b`. Group descriptions, sidebar order holds, pin drag and Shift-drag passed 21 tests
    in `0051ec8e-1646-4e30-a28b-d4835d91bad2`.
  - #4477, content in `f0cf6c0112ad`. Hosts attention rows and cues passed 22 tests in
    `6e2786f8-a2f3-44fb-8f87-b2106993f9ba`.
  - #4481, content in `47fdf20ec39f`. Environment actions, Delete and Stop confirmation, and unavailable session cards
    passed 43 tests in `bc9e8cec-ae56-4e83-925c-8d44c8cd1ff5`.
  - #4486, content in `90b479a9407d`. Attention cues, notices, the attention list, environment actions, session panes,
    SSH sign-in and Trash passed 126 tests in `73d553c4-29da-4c39-b1f6-522d9256d820`.
- For each visual draft, targeted `python3 scripts/check-gui-goldens.py --update --scene …` and a full
  `python3 scripts/check-gui-goldens.py --update` produced matching changes in both appearances. I inspected the changed
  images directly before publishing them to the golden reference store. The drafts carry their manifest entries.
  - #4477 updated two references for `host-rail-status`. Targeted `fac8dd68-708d-43de-ae94-780e6014016d`, full
    `4bfe9d56-7ab2-4130-84f4-04776d751b3f`.
  - #4481 added six references for `session-environment-unavailable`, `environment-delete-confirmation` and
    `tab-only-stop-confirmation`. Targeted `ae0881bb-2faf-4096-ad72-7ac4ad31c25b`, full
    `34e00cc3-f7cc-41c3-900c-e19a15636213`.
- Workspace-wide tests were skipped because the focused GUI suites cover the changed behavior. Visual review applied to
  the two drafts that changed painted views.

## Review gates

Every draft passed the independent high-effort GPT-6.2 Sol source review and the commit and PR wording cold read. A
fresh high-effort Opus review inspected the changed light and dark renders in each visual draft and found no remaining
mismatch.
