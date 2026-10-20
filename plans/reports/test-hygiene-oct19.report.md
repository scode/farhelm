## What this was about

Four test defects could produce misleading failures or contaminate later tests: CLI checkout assertions compared two
spellings of the same temporary folder on macOS; notification cleanup stopped after its first error; named shortcut and
checkout-naming cleanups could skip later resources and shared settings; and focus tests created a session before arming
cleanup for observer setup.

All four agreed outcomes now have separate draft PRs. The checkout expectations use the resolved fixture root. Cleanup
attempts independent resources while retaining an earlier test error, and the focus tests own cleanup immediately after
creation. The measured code, test and documentation diffs are 20, 67, 99 and 10 lines, excluding execution records and
feedback removal. Each meets its 100-line gate.

## Things you should know

These are test-only changes. The shortcut fix covers only the eight cleanup blocks named in triage, producing nine
cases; the other similar blocks remain outside its agreed scope. Session lists are read after pending routes drain, so a
late accepted create is included in cleanup. Each PR removes its feedback item and records its execution; no changelog
fragment is required.

The notification regression injects route and session cleanup failures into the same local orchestration used by the
real test hook. It proves that later attempts and preference restoration still run, ownership is drained, and the
original body error remains primary. Ordinary browser cases separately cover the real page and API callbacks. Shortcut
cases received ordinary runtime coverage and static inspection of their independent callbacks, not fault injection at
every call site.

## Open questions and possible follow-ups

None required for this plan. macOS was unavailable: the checkout correction was checked statically against its symlink
behavior and executed on Linux. The browser engines cover the web UI, not the native desktop shell.

## The PRs

- [#2429](https://github.com/scode/farhelm/pull/2429/changes): correct the CLI checkout path expectations; head
  `90207ea50a634168c3a7f91c6de90af182e44be1`.
- [#2432](https://github.com/scode/farhelm/pull/2432/changes): finish notification cleanup after earlier failures; head
  `6f2afc4110daf8ed953a60987db9c9b54459d907`.
- [#2434](https://github.com/scode/farhelm/pull/2434/changes): independently attempt the named shortcut and checkout
  cleanup; head `a5d60c39000707c36b75e42668cc0c13b999f2f1`.
- [#2435](https://github.com/scode/farhelm/pull/2435/changes): arm session cleanup before focus-observer setup; head
  `25dfd40a49d1a91549de6dabea4a5218a44703ef`.

The stack is linear in that order, starting on main. Every PR remains a draft for the landing monitor.

## Checks run, reused and skipped

Runtime checks used the recorder, zero retries, pinned nextest 0.9.148 and tmux 3.7c where applicable, four nextest
slots or one browser worker. Each retained record has complete source capture, output through EOF without truncation,
and completed cleanup. Browser reports and both-engine policies passed. No runtime substrate skip was observed.

| Run ID                                 | Selection and result                                                                                                                                                                                                                                                                                                         |
| -------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `60a97fba-bddb-4b6f-9667-a136a24df82d` | `cargo nextest run -p farhelm --test e2e -E 'test(=github_checkouts::agent_create_checkout_replays_and_templates_use_fresh_admission)'`: one selected integration case passed; fixture build setup also passed. JUnit counts both as two passes, with 390 cases selected out.                                                |
| `050d8e22-62a6-4e73-a7cd-e6d7a073382d` | `npx playwright test 'notifications\.spec\.ts'`: 18 passed, nine per engine, including the controlled cleanup-failure case on each engine. Each controlled case checks four failure scenarios.                                                                                                                               |
| `a1d98fad-3ce0-41d0-a5d8-279c4337ed07` | The three changed shortcut and checkout spec files, restricted to the nine named cases: 18 passed, nine per engine.                                                                                                                                                                                                          |
| `c762b3bf-22a0-46ce-83a4-e3881bc09691` | `npx playwright test 'terminal-replay-rename\.spec\.ts' -g 'replay reveal preserves a header\|selected tab removal preserves a header delete question'`: six passed, three per engine.                                                                                                                                       |
| `441b0469-1758-4c4e-b8f0-24941664e5c3` | A private Deno source probe extracted the actual focus-test callback bodies and injected observer rejection through callbacks. Two checks passed, covering all three variants and a negative control that restores the old cleanup gap. This is callback execution, not a real browser setup failure or native desktop test. |

The CLI run captured base `cd24572f` plus the first fix; the notification run captured first-PR head `90207ea5` plus the
second fix; the shortcut run captured second-PR head `6f2afc41` plus the third fix; the focus browser and probe runs
captured third-PR head `a5d60c39` plus the fourth fix. Subsequent edits to each reviewed change only added its PR URL
and size to the execution ledger. Those runtime results remain applicable. The private probe's script identity is
recorded in the working log separately from the repository fingerprint.

`cargo build` and the Dioxus 0.7.10 release web build passed for the unchanged production sources at `cd24572f`. Those
prerequisites were reused across the browser runs because later edits affected only test source and bookkeeping.
`cargo fmt --all -- --check` passed after the Rust assertion change and remained applicable through the later TypeScript
changes. Changed Markdown formatting, `check-agents-md.py`, and the isolated test-sleep checker passed; the latter
inspected 305 delays with no missing rationale. Deno formatting and type checking passed for the private probe. The
portable five-run summary has complete discovery and no coverage gaps or failed commands; its generic Deno command has
console results rather than runner-reported case counts.

Main advanced from `cd24572f` to `b4db0c7b` only through other plans' completion states and reports. The full
intervening diffs were read; none changes the tested behavior, so no rebase or additional runtime check was warranted.
Broader Rust, browser, desktop, installer, website and hosted CI runs were skipped because these scoped test repairs
have direct coverage and change no product behavior. No failure or retry occurred in this plan.

## The review gate's outcome

Each PR passed its required single fresh-context gpt-6.1-sol/high review with the full test-authoring checklist. No
findings remain. Commit and PR wording passed separate cold reads. No material scope expansion or size fallback was
needed. A separate cold read checked this report before delivery.
