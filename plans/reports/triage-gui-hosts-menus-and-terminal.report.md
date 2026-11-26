# Host confirmations, Templates, session menus, and terminal notices

## What this was about

When Farhelm cannot uninstall from a host and offers Remove as the remedy, the button could forget the host without
asking if “Remove hosts without asking” had just been enabled. Turning “Set up new hosts without asking” off during an
SSH Add or Re-run check could still let installation start without confirmation. Turning “Create Tensorlake sandboxes
without asking” on during a Tensorlake Re-run could similarly let the billed creation start without asking.

A click on the old Templates footer could save or duplicate the next editor, including saving settings Farhelm inferred
for a legacy template. The row menu's Stop and suspend still offered to stop a program that had already ended. A click
on a terminal file path gave no explanation when Farhelm hid the details panel because it covered the pointer. The test
for an early click checked only the moment before the panel was placed, leaving a gap in coverage for a spurious refusal
before the panel appeared. Repeated window warnings could stay scrolled out of view.

## Things you should know

- Remove uses the host and confirmation preference from the frame you saw. SSH Add and Re-run ask if confirmation was
  required at the press or when the host check finishes. Tensorlake Re-run reads the account at the press and again
  after the host check. It shows the billing confirmation if either read calls for it or fails. Triage allowed each
  setup timing fix only if it stayed simple, with a TODO follow-up otherwise. Both fit the existing host checks.
- Templates Save and Duplicate use the editor that drew the footer. The session menu describes closing the remaining
  tabs and suspending the environment after the main program has ended. For an ended session with no tabs, it describes
  finishing Stop and suspending the environment. The request still uses the guards from the row you saw.
- If a file details panel is hidden because it covers the pointer, clicking the path asks you to enlarge the window and
  hover again. A different path under a resting pointer starts a fresh check. The early-click test now clicks after the
  panel is placed and before it paints, and checks that there is neither a transfer nor a refusal. Its 14-line diff fit
  triage's 20-line allowance for test-only fixes protecting user-visible behavior.
- Reposting a notice moves its single banner to the bottom and scrolls it into view without changing keyboard focus.

## Open questions and possible follow-ups

- Update the pre-existing Stop and suspend confirmation for a running program to say that Resume continues the processes
  that survive Stop. The current copy promises every other process will resume, although Stop can reap surviving
  descendants of the program it stops.

## Pull requests

- Host removal and setup confirmations. [PR #5033](https://github.com/scode/farhelm/pull/5033/changes)
- Templates Save and Duplicate, and Stop and suspend wording.
  [PR #5036](https://github.com/scode/farhelm/pull/5036/changes)
- File details holdback, the pre-paint test, and repeated notices.
  [PR #5039](https://github.com/scode/farhelm/pull/5039/changes)

## Checks run, reused, and skipped

- The GUI test commands used recorder-backed `cargo nextest run --locked -p farhelm-gui --lib -E <selection>`. The
  comparisons used recorder-backed `python3 scripts/check-gui-goldens.py --scene <selection>`.
- Host confirmations. The Add, setup, removal, environment, Update, and Uninstall test modules passed 148 of 150
  selected cases at `e7ad224d` (run `5e73d2c0-a8c0-452a-b7b4-d827a44413a5`). The refusal-click fixture failed because it
  lacked an initial preferences row. The Tensorlake probe refusal fixture failed because it expected only the probe
  request. Both passed after fixture corrections in the six-case refusal and Tensorlake recheck at `a7fa859e` (run
  `a376ddd1-6c19-4e26-a943-5df9f9bdb678`). The first run covers the unchanged host paths, including the SSH confirmation
  cases.
- Templates and session menus. The editor, leave-question, session menu, and environment test modules passed all 123
  selected cases at `889bc5ee` (run `83b74015-444b-45d5-9799-2f0746f02c35`). The five-case recheck of the relevant
  Templates and Stop and suspend behavior passed at `db2334a6` (run `40932d75-517e-4818-a285-d1c422697d6e`). The first
  run covers the paths untouched by the review changes. The selected Linux screenshot comparisons matched at both
  revisions (runs `69cbdab4-bc21-4931-9bba-614fb682ca71` and `7419faba-4e65-4304-a45c-35b4062d27ca`).
- Terminal files and notices. The file-disclosure module, small-window web-link refusal, overflowing notice stack, and
  What's New tests passed all 40 selected cases at `73e1a666` (run `5b7e3792-cc2a-46b3-bbfc-a1f97b29fbd0`). The file,
  link, and restored-notice Linux screenshots matched (run `6cd2a6c9-b099-4391-93f3-fc1d6ef9ee9b`). The 34-case terminal
  recheck at `be00e72d` passed (run `388553c7-ee5b-4e3b-8b7a-52dd2f00efd3`), and the selected terminal screenshot
  comparisons matched (run `4a2821d0-afdd-4ee3-b617-dff948178c6f`). The first run covers the notice changes and their
  screenshots.
- Strict GUI Clippy (`cargo clippy --locked -p farhelm-gui --all-targets -- -D warnings`), Rustfmt, the test-delay
  checker, and changelog format checks passed on the validated candidates. Dprint matched the new archive entries and
  the other edited Markdown. Two older archive wrapping findings match main's baseline and remain in the frozen archive.
- The local checks used a fake helm and the Linux software renderer. Full workspace tests, the full screenshot catalog,
  native macOS, and live host provisioning were skipped. The selected cases exercise the affected desktop actions and
  rendering, and the landing gate supplies the broader check.

## Review gate

One fresh GPT-6.2 Sol high source review per pull request. Addressed the review findings in the affected code and tests,
including the refusal footer's redraw dependency, ended-program wording, and stale file-disclosure evidence.

### Landing

#### What landed with it

This plan landed second in its round, right after the agent request and container approval triage
(`triage-security-and-agent-boundaries`), which changes only the helm and shares only the triage records with it. Since
this plan's PRs were based, main gained the Restart and hook warning fixes, the release brick test fixes and a TODO
entry. The rebase met only conflicts in the shared triage records, kept both ways, and the landing's independent review
found no interaction. It also found the plan's behavior sound: no confirmation choice is ignored or applied to the wrong
host or row, every failed or out-of-order read falls back to asking, and a reposted notice cannot loop.

While this plan's PRs were merging, another change reached main between its first and second PR: a test-only change to
the screenshot comparison that fails a render that hangs ([#5045](https://github.com/scode/farhelm/pull/5045/changes)).
It touches only the screenshot harness and its runner, which this plan does not, so main now holds the tested tree plus
that change.

#### Fixes made while landing

- None. The review found no blocker or should-fix item.

#### Checks

Worker runs were on a separate arm64 Linux test worker; the rest ran locally on x86_64 Linux. All the checks below ran
on both plans' PRs stacked in queue order on main as it stood at the start of the landing (`1e344451`), with the shared
triage records' conflicts kept both ways; every other change is exactly what the executors pushed. The round's one
landing fix since, inside the agent request and container approval plan's #5030, removes a blank line from the review
feedback queue's index, so these runs still apply.

- The workspace Rust suite, `cargo nextest run --workspace --exclude farhelm-desktop`, on the test worker (recorder run
  `3ab311fb-56ce-4f7e-a781-048ff52d03ee`): all 8755 passed, with no runtime `SKIPPED` lines. It covers both plans' helm
  and GUI tests.
- `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings`,
  `cargo clippy -p farhelm-desktop --all-targets -- -D warnings` and
  `cargo clippy -p farhelm-desktop --all-targets --features agent -- -D warnings`, on the test worker: passed.
- The desktop app's end-to-end smoke, `python3 -B scripts/gui-e2e-smoke.py`, on the test worker (recorder run
  `da502306-29fe-48a5-af71-405b24619819`): of its 49 steps, 48 passed, including Rename, the spawn approval steps, the
  host menus, Stop and the templates, and one (paste) was skipped by design on Linux.
- The desktop app's screenshot comparison, `python3 scripts/check-gui-goldens.py`, all scenes, locally (recorder run
  `5ec3eec6-72fa-4964-9cd6-17b84713fc75`): passed; neither plan changes a reference picture.
- `python3 scripts/check-agents-md.py`, `python3 releasing/check-changelog.py format`, `cargo fmt --all -- --check` and
  `python -B scripts/check-test-sleeps.py`, locally: passed. `dprint check`, locally: after the fix it fails only on
  over-long lines in the triage outcomes archive, main's earlier ones (#4978) and the security triage plan's three new
  entries. No PR fixes them yet; a formatting-only one would, and I recommend it, as earlier rounds' notes did.

Reused across the fix: every run above except `dprint check`, which ran again, since the fix changes only whitespace in
the queue's index. The executors' runs were on older bases, and everything they covered ran again above.

Skipped: the desktop app's outer shell tests, since nothing in that crate changed; a Mac run; real Podman and Tensorlake
hosts, which no automated check here reaches; and CI, which runs only on demand.

#### What the report says that is no longer true

- Nothing.

#### Things you should know that the report does not say

- **Smaller points the review noticed.** Tensorlake's Re-run sends its settings read even when it then does not start
  because other work is running on the host, though its own description says nothing is sent then (harmless); the
  confirmation changelog draft says "turning it off" where it should name **Create Tensorlake sandboxes without
  asking**; the Stop and Suspend wording change adds a variant for an ended session without tabs ("Finish stopping this
  session") beyond what triage asked for; one changelog draft uses a curly apostrophe; and clearing the file details'
  waiting-for-hover state while drawing could use a comment. None needs a follow-up on its own.
