# Queued quick launch keeps the displayed template choices

## What this was about

An armed quick-launch sequence could show one template and launch another after an agent or another client changed a
number shortcut or the quick-launch mark. That could start the session on a different host or in a different folder from
the template shown. The maintainer accepted fixing this only while the change stayed small and localized.

Enter now captures the template names shown by the sequence before fetching current settings and template definitions.
Moving a shortcut elsewhere cannot substitute another template. A key shown as unbound still refuses even when the fresh
settings give it a binding. The feedback entry was removed and its execution recorded.

## Things you should know

The complexity gate held. This uses the existing sequence, dispatch and refusal paths; it adds no stored snapshot or
protocol change. Order and repeated templates are preserved. An unbound number or mark retains its existing binding hint
on the page and refusal in an open launcher.

Only template names are captured. Their definitions are still read fresh, so an edit under the same name applies; a
renamed or deleted template refuses rather than launching the old definition. Single-chord quick launches retain their
fresh binding lookup.

## Open questions and possible follow-ups

None required for this plan. This does not change the separately accepted behavior of templates edited or recreated
under the same name.

## The PRs

- [#2355: keep queued template choices when shortcuts change](https://github.com/scode/farhelm/pull/2355/changes) —
  draft, one reviewable change for the outcome.

## Checks run, reused and skipped

- `cargo fmt --all -- --check` and `cargo clippy --all-targets -- -D warnings` passed.
- Recorded `cargo nextest run -p farhelm-ui --lib -E 'test(launch_sequence::tests::)'` passed all five selected tests;
  542 other tests were selected out. Recorder run `b6ef984c-9f5e-4618-a900-6680a5fb18b2` used four slots, zero retries
  and no tmux because these are pure unit tests. The two new cases cover moved number and mark bindings, newly bound
  references, sequence order and repetition, fresh definitions and missing-name refusal. Source, output and JUnit
  evidence were complete; selected stdout had no runtime skips, and cleanup required no forced termination.
- Targeted `dprint check` passed for the specification, ledger, feedback index and fragment.
  `python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py` and the isolated
  `python -B scripts/check-test-sleeps.py` passed.
- No earlier plan's check results were reused. The final amendment only added the PR link to the ledger; an interdiff
  confirmed no source change. Main moved only through queue bookkeeping, which was inspected and has no interaction.
- Browser, desktop and JavaScript runs were skipped: no DOM, event capture or JavaScript code changed. The synchronous
  Rust handoff and pure reference resolution are covered by the selected tests and code review. Supervisor, SSH and
  systemd runtime suites do not exercise this boundary.

## Review gate's outcome

A fresh-context gpt-6.1-sol reviewer at high effort found no correctness, design, idiomaticity, test-authoring or scope
issues. The commit message's separate cold read matched its intended meaning and found no contradicted claims or
convention violations. A documentation pass covered every touched file.

### Landing

Landed as #2355, unchanged from what the report describes. It was the fourth of eight plans in this landing round;
launch-enters-working-dir (which makes sessions enter their folder or fail) failed its checks at landing, was blocked,
and is not part of what was tested with this plan.

#### What landed since the stack was based, and whether it interacted

Since the stack's base, main gained link-hover-covers (link clicks refused under the Mac app's tunnel-error notice, and
the desktop file-opening fix), workspace-trust-destination (inherited spawns take trust from their destination, and a
trust note on approval cards), session-header-and-row-fixes (session questions keep the keyboard during terminal
recovery; copy warnings; accessible names), and plan bookkeeping. A fresh-context reviewer and I read this plan against
those and against the other plans in the round: none of them touches quick-launch sequences, number shortcuts, the
quick-launch mark or how a launcher applies a template, so nothing interacts.

The only conflicts were in the review feedback queue's index, where every plan in the round removes its own entries; all
removals are kept. The landed code is identical to what the landing's checks below tested.

#### Fixes made during the landing

None.

#### Checks

Run now, on one tree holding this plan together with the round's other landing plans, before and after it in landing
order:

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings`, `cargo check -p farhelm-ui --features desktop`, the isolated
  test-sleep check, `dprint check` on every changed file, and `python3 releasing/check-changelog.py format`: all passed.
- Through the test recorder with pinned nextest and tmux, four slots and no retries, the whole protocol, helm,
  supervisor and UI libraries, the `farhelm` binary's tests, the agent command-line tests and the end-to-end tests for
  launches, tabs, checkouts, agent relay and session lifecycle: 3489 passed, run `cdabc40d-ecaa-418e-938f-01f2cbe4da64`.
  The UI's desktop-only Rust tests: 706 passed, run `4926be34-2adf-40e5-a588-663695e00161`. `node --test` in the UI's
  JavaScript test directory: 311 passed, run `fcb2b9cf-4424-4fc2-8148-dede8454a5ea`.
- After `cargo build` and the release web build, `npx playwright test` on the whole quick-launch and launcher
  template-shortcut files and nine other files for the round's other plans, on Chromium and WebKit, one worker, no
  retries: 242 passed, run `1484ba9a-0aab-4e7f-a7a4-716faede7c66`. Those two files cover armed sequences, number
  shortcuts and the quick-launch mark.
- Skipped: the rest of the browser suite and the native Mac app; this plan changes only what an armed sequence launches
  when Enter is pressed.

Nothing in the executor's report became untrue at landing.
