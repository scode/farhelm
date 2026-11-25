# Environment launches, wake, and cleanup

## What this was about

The November 24 environment triage covered the launcher, session removal, Stop and suspend, Trash, and Tensorlake
cleanup. Several findings concerned the choice Farhelm acts on when a host wakes or a checkout changes. Others concerned
warnings that arrived after the original reply timed out, or actions that stayed available while their target was being
removed. This stack records a disposition for all 31 findings.

## Things you should know

- A visible Launch press keeps the folder you saw before the wake. A hidden quick launch waiting for a recent folder
  discards history for an old connection and asks again for the current one. A new environment asks you to select a
  repository before it promises a clone, and can use repository history even when the source host needs checkout setup.
- Restart with now reports wake failure as a restart refusal. A managed checkout shows why it cannot use an unverified
  host. If a preview for the newly connected installation fails and ends preparation, the launcher returns with that
  reason. A fresh checkout inside an existing disposable environment says its folder will be removed permanently after
  its last session is deleted. The separate Replace with warning still describes retiring the source checkout.
- Ordinary Replace with and environment Delete keep a checkout warning that arrives after their respective reply
  deadlines. The warning goes to the notices bell and identifies the source session. Stop and suspend correctly
  describes the tabs that will close after the main shell has already exited. Its activity read leaves the write queue
  free. Trash drops an old host confirmation when the host goes away or a settled refresh shows no items, and stops
  running the nav-button animation while hidden.
- Tensorlake leftover cleanup refuses to proceed under a different API key. Restoring the original key makes cleanup
  possible again. Setup and Update refuse a host that has already been sealed for environment deletion.

The final rebase was checked against main's global repository suggestions and refreshed checkout setup. It preserved the
newer restriction that rejects an invalid checkout title before provisioning, and it verifies that a fresh Replace with
can teach global repository history and wake Trash without adding an ephemeral folder to host history.

## Open questions and possible follow-ups

- The triage decision was to fix harmful actions based on stale frames. Two Replace with routes that can start a fresh
  clone before the warning is shown share one TODO. The attempt found that a warning can be laid out, clipped or
  replaced before presentation. Consent must stay tied to the actual checkout through the host wake and preview. The
  complete change exceeds the triage budget.
- The TODO also tracks late or repeated wake failures and carrying one-time YOLO consent through a wake. The former
  needs a reliable outcome for the exact wake, including a later provider failure with no state change. The latter needs
  to preserve the accepted permission only for the original installation, even if the saved host row moves.
- An old Settings sandbox Delete prompt can still submit against a replacement Tensorlake account. A key write may
  succeed even if its reply is ambiguous. The TODO covers that case and calls for a fresh listing before Delete is
  offered again. The new cleanup guard above covers retained cleanup records, while this Settings case concerns the
  answer to a previously displayed prompt.
- A late warning after an **agent's** Delete needs to follow the asking agent's actual deadline, which can differ from
  the helm's deadline. That correlation is tracked in TODO. Editing a recipe before admission can make the sandbox bill
  for more CPU and memory than the confirmation showed. The TODO leaves open whether older clients may launch without
  supplying the size they confirmed.
- A keyboard user still cannot reach the clipped environment name in the Delete prompt. Missing test coverage includes
  whether prepared Tensorlake Add keeps the source snapshot protected from retirement until the sandbox is running, and
  whether successful agent-requested environment launches stay out of recent setups and default places through the full
  request paths. These cases exceeded their small triage budgets. The full Replace-with-checkout wake matrix remains
  open. Restricted connections already have a test proving that whole-environment Delete is refused. Main's checkout
  parity notes still name the old repository scanner in one section.

## Pull requests

- Record the two fresh-clone warning findings together and defer presentation-bound Replace consent.
  [#4886](https://github.com/scode/farhelm/pull/4886/changes)
- Ask for the repository before promising a clone in a new environment.
  [#4891](https://github.com/scode/farhelm/pull/4891/changes)
- Preserve a visible launch's folder and recover a hidden launch from stale history after wake.
  [#4898](https://github.com/scode/farhelm/pull/4898/changes)
- Record the late and repeated wake failure follow-up. [#4901](https://github.com/scode/farhelm/pull/4901/changes)
- Record the one-time YOLO wake follow-up. [#4910](https://github.com/scode/farhelm/pull/4910/changes)
- Label a failed Restart with wake as a restart refusal. [#4916](https://github.com/scode/farhelm/pull/4916/changes)
- Explain the unverified checkout target and cover its refusal and stopped-host wake.
  [#4918](https://github.com/scode/farhelm/pull/4918/changes)
- Restore the launch refusal when a fresh checkout preview fails during wake.
  [#4922](https://github.com/scode/farhelm/pull/4922/changes)
- Explain checkout removal in an existing environment, cover a folder wake deadline, and record the remaining wake test
  gaps. [#4925](https://github.com/scode/farhelm/pull/4925/changes)
- Retain late Replace with and environment Delete checkout warnings in the notices bell.
  [#4929](https://github.com/scode/farhelm/pull/4929/changes)
- Clarify Stop and suspend after shell exit and let its activity read run separately from writes.
  [#4931](https://github.com/scode/farhelm/pull/4931/changes)
- Bind retained Tensorlake cleanup to the original key and record the prepared Add coverage limit.
  [#4937](https://github.com/scode/farhelm/pull/4937/changes)
- Release stale Trash confirmations and stop hidden Trash animation.
  [#4939](https://github.com/scode/farhelm/pull/4939/changes)
- Refuse Setup and Update while environment deletion holds the seal.
  [#4942](https://github.com/scode/farhelm/pull/4942/changes)
- Record the Settings sandbox Delete follow-up for key changes.
  [#4947](https://github.com/scode/farhelm/pull/4947/changes)
- Record the keyboard-access disposition and the late agent Delete notice follow-up.
  [#4948](https://github.com/scode/farhelm/pull/4948/changes)
- Record the billed-size confirmation follow-up. [#4949](https://github.com/scode/farhelm/pull/4949/changes)
- Prove checkout borrower membership and an archive wake from Replace. Close the restricted Delete and launch-history
  findings with their test dispositions. [#4953](https://github.com/scode/farhelm/pull/4953/changes)

## Checks

- Ran pinned `cargo nextest run --locked -p farhelm-gui --lib -E <selection>` on the rebased stack at `99bdc8ba`. All 98
  selected tests passed (run `0e6ab965-6b14-418a-98cf-a66953ef979b`). The selection covered `launcher::checkout_tests`,
  `launcher::wake_tests`, `launcher::replace_with_tests`, `launcher::folder_default_tests`, and the
  `a_suspended_quick_launch` and `a_quick_launch_without_a_folder` cases.
- Ran pinned `cargo nextest run --locked -p farhelm-helm --lib -E <selection>` on the same rebased stack. All five tests
  selected by name passed (run `30592844-a180-4df8-a7ab-bde3474f4357`). They cover the environment home fallback,
  archived Replace with, and timely and late removal notices.
- Ran `python3 scripts/check-gui-goldens.py --update --scene 'launcher-*' --jobs 4` on the same candidate. All 58 light
  and dark pictures matched the published reference blobs byte for byte. Every new reference in the stack was inspected
  in both appearances and published after a complete four-process render of its reviewed revision. This rebase needed no
  reference update.
- Reused the six exact Helm removal tests at `5ebb83e0` (run `28f9e698-7210-4b35-a80e-a52e8aefda47`) for the deadline,
  notice routing and relative placement of the locks. The rebased run retested the cross-plan checkout and history
  interactions. Reused the focused shell-exit and activity cases at `6ba76881` (144 passed,
  `2283f0ee-0046-4785-ad6d-2a64686780bc`), Tensorlake cleanup and Settings at `45851a64` (16 passed,
  `52bb00a5-d65f-4cba-afe2-9b0f2eeba4a3`), Trash at `f1302044` (54 passed, `82eb2c45-c5ee-4920-ab7c-49920c67416e`), and
  sealed provisioning at `74d3e856` (six passed, `3a46b1d2-07c1-42a1-8db2-1657528dc417`). Both the rebase and the
  upstream changes left the tested behavior intact. Provisioning Clippy passed at `7512261e`, whose production code
  matches the checked provisioning revision.
- Reused the borrower and restricted-dispatch evidence from `10b13458` (four passed,
  `c4919ba7-6876-483c-b2dc-6d98d903c464`). Those assertions and routes carried through unchanged. The rebased Helm run
  retested the archive witness alongside the new global-history assertions.
- Rust formatting, changelog format, test-delay rationales and whitespace passed on the rebased tree. Instruction links
  found one existing archive reference to a heading that no longer exists, with zero errors.
- Skipped the full runtime battery and live provider runs. The changed interactions have focused GUI and Helm coverage
  using controlled peers. Skipped docs-shot recapture because the GUI work removed or covered no crop or pointer park
  target. Documentation-only dispositions use formatting and link checks. Prototype test results are excluded from
  delivery evidence.

## Review

The independent Sol code review and Opus visual gates passed for the delivered changes. Reviews strengthened exact
installation and deadline assertions, checked what the launcher had actually shown before Enter, and added a notice-only
Trash case. Light and dark references were reviewed together. A fresh Sol pass found no blocking integration issues on
the rebase. It noted inherited repository-scan documentation drift.
