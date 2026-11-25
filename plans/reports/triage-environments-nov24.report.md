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

### Landing

#### What landed with it

Only this plan landed this round, through its eighteen PRs and the landing fix
[#4961](https://github.com/scode/farhelm/pull/4961/changes). Since the plan's PRs were written, main gained the sandbox
workspace trust plan (#4943, #4955, #4958), which landed in the previous round: launches into a session's own sandbox or
container now get workspace trust automatically, and recipe host setup fills in Claude Code's first-run setting. Both
plans change how the helm creates sessions in sandboxes and how it provisions hosts, so the landing's review read them
against each other and found no interaction:

- **Launches that wait for a host to wake.** This plan changes several of them (a visible Launch press, a quick launch,
  Restart with, a managed checkout's preview after a wake). Each reaches the helm only after the host has reconnected,
  and the helm then decides workspace trust from the connection it creates the session on, so a destination that changed
  during the wake is judged afresh.
- **Setup and Update refused during environment deletion.** This does not collide with the new Claude first-run step,
  which runs as part of credential delivery when a host is added or connects, not through Setup or Update.
- **Specs.** The two plans' SPEC.md and SPEC_impl.md edits are in different paragraphs and agree.

The rebase applied without conflicts, and a file-by-file comparison confirmed every file's change is exactly what the
executor pushed. Before rebasing I removed stale copies of the plan's commits that an earlier fetch, made while the
executor was pushing, had left in the landing checkout; they were older revisions of the same commits and never part of
what landed.

The review counted no blockers, no should-fix items and six small points. Three are fixed below (the two changelog
drafts and the code comment); the missing scene list entries were found by the checks, not the review. The other three
are the last three items of the last section.

#### Fixes made while landing

- **Eight new screenshots were missing from the screenshot scene list.** The plan added golden pictures of its new
  launcher, Stop, Trash and Tensorlake cleanup states, but not their entries in the scene list of the screenshot batch
  tool (the side-by-side page of the app's screens next to the old app's), so that tool's test failed on the plan's
  final revision. #4961 adds them.
- **Two changelog drafts misstated their scope.** The draft for the late checkout notice limited it to disposable
  environments, but it also covers Replace with on any host; the draft for the checkout lifetime note limited it to
  Replace with, but it appears for any new managed checkout inside an existing sandbox or container. #4961 corrects
  both, so the release curation does not copy the narrower claims.
- **A stale code comment.** The comment on the launcher's wake refusal note still described a complete refusal, though
  the launcher now adds the "Not launched." or "Restart error." lead itself and the note also carries a failed checkout
  preview after a wake. #4961 updates it.

#### Checks

Worker runs were on a separate arm64 Linux test worker; the rest ran locally on x86_64 Linux. All the checks below ran
on the plan's PRs rebased onto main as it stood at the start of the landing (`455d5f00`), without conflicts; the rebased
stack's top commit was `5923a344`. The only later change is the landing fix
[#4961](https://github.com/scode/farhelm/pull/4961/changes), which adds eight entries to the screenshot scene list and
corrects two changelog drafts and one code comment; after it I ran the scene list's test again (recorder run
`1b16bdfc-56b2-48a5-94ff-d2c97b6870a8`, passed) and the formatters and the changelog check, which passed. That final
tree is what main now holds outside the plans queue.

- The workspace Rust suite, `cargo nextest run --workspace --exclude farhelm-desktop`, on the test worker (recorder run
  `0ac3e74c-40b1-4590-b68b-131c6d761933`): 8672 of 8673 tests passed, with no runtime skip. The one failure,
  `codex_subscription_and_none_enter_setup_without_an_endpoint` in the helm's Tensorlake Add tests, is a known flake on
  main, not this plan's: run alone ten times on the test worker it failed 3 times on this plan's tree and 3 times on
  main's tree without the plan (recorder runs under the selection "tensorlake_add
  codex_subscription_and_none_enter_setup_without_an_endpoint", for example `c2361467-e105-43a7-82bd-bdf634b8f0fa` and
  `d980e1a7-7c6f-447f-9c89-b42e5b3f57df` failing on the plan's tree, and `74684414-d0a4-4527-844f-2df823127ddd` failing
  on main's). The last section says more. The plan changed the helm's sessions, sandbox, provisioning, agent requests
  and Tensorlake leftover cleanup, the supervisor's create path, and the GUI's launcher, session list, Trash and
  Settings, with their tests; the whole suite was the narrowest run that covered them and their callers.
- The desktop app's tests, `cargo nextest run -p farhelm-desktop`, on the test worker (recorder run
  `8764b7c2-4ae1-43d6-b564-ab59de279eac`): all 184 passed.
- `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings`,
  `cargo clippy -p farhelm-desktop --all-targets -- -D warnings` and
  `cargo clippy -p farhelm-desktop --all-targets --features agent -- -D warnings`, on the test worker: passed.
- The desktop app's end-to-end smoke, `python3 -B scripts/gui-e2e-smoke.py`, on the test worker (recorder run
  `8c76f477-b3b8-461b-800e-54b5c3c6ae06`): 50 steps passed, one skipped by design on Linux (paste). It drives the
  launcher, Stop and Trash.
- The desktop app's screenshot comparison, `python3 scripts/check-gui-goldens.py`, all scenes, locally (recorder run
  `41ea61e8-6a5e-412e-a6f4-0c055bd8107c`): passed, including the plan's eight new scenes.
- The screenshot batch tool's test, `python3 -B scripts/test-gui-screenshot-batch.py`, locally: failed on the plan's
  stack (recorder run `6f7fcb81-cf3d-4107-b160-16cc832ea526`) because the eight new scenes were not listed, and passed
  with the landing fix.
- `cargo fmt --all -- --check`, `python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py` (no
  errors; one warning, about an old citation in the triage archive, already on main),
  `python -B scripts/check-test-sleeps.py` and `dprint check`, locally: passed.

Reused: none. The executor's runs were focused selections on the older base, and everything they covered ran again
above.

Skipped: live Tensorlake and container launches, which need provider accounts; the workspace doctests, since no
documentation example changed; a Mac run, since nothing changed is Mac-specific; and CI, which runs only on demand.

#### What the report says that is no longer true

- Nothing. The executor did not run the screenshot scene list's test, which failed on the plan's final revision as
  above, but the report does not claim it passed.

#### Things you should know that the report does not say

- **A Tensorlake setup test on main now fails about a third of the time.** The workspace run's one failure is
  `codex_subscription_and_none_enter_setup_without_an_endpoint`, which adds a Tensorlake host whose recipe uses a Codex
  subscription login, or no Codex login, and checks what the host's record of delivered credentials says about Codex
  afterwards. It is the flake recorded in the draft flake-record PR
  [#4757](https://github.com/scode/farhelm/pull/4757/changes), still open for you. That PR's suspected cause, not yet
  confirmed, is that the credential delivery that runs when the new host connects can write to that record before the
  test reads it. It failed 2 of 25 runs when that PR was written; here it failed 3 of 10 both with and without this
  plan, so this plan does not cause it, but something since then has made it fail more often. I suspect, without having
  checked, the previous round's Claude first-run step, which adds another write to that same delivery. I recommend
  merging #4757 and fixing the test's wait; say if you want me to update #4757 with these numbers.
- **Replace with's timeout message does not point at the notices bell.** When Replace with takes too long to answer, its
  message says the session list will show the result, but the bell is now the only place the left-behind checkout's path
  appears. Delete's timeout message does point at the bell. A one-sentence wording change; say if you want a TODO entry.
- **One test pins a reply a real sandbox never sends.** The new Replace with Trash test has a sandbox or container
  report the source checkout as archived, which never happens there, since Delete in a sandbox discards the checkout. It
  checks the right refresh, but a correct later change that skipped the Trash refresh for such hosts would fail it. A
  follow-up could have the test use a host that can archive; I judge it minor, so say if you want a TODO entry.
- **A Launch press in the first frame after opening can do nothing.** If it lands before the untouched default folder
  has been drawn, the press is ignored without feedback, on purpose, so that it can never launch into a folder the user
  did not see. The review judged that acceptable.
