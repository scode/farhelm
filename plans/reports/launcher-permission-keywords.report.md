# Launcher permission choices and warning colors

## What this was about

Typing `yolo` before choosing an agent answered “Choose a harness first” in ordinary text, which looked like a broken
feature. The permission buttons already allowed that order. The requested change was to make launcher rejections
conspicuous in yellow, allow the same permission choices through search, and reserve red for danger.

The launcher now offers Guarded and YOLO before an agent, through their buttons, bare words and `perms:` words. If
choosing the first agent replaces an explicitly chosen permission with a more dangerous one, the existing red banner
appears and Undo restores the earlier choices, including having no agent selected.

## Things you should know

Yellow applies throughout the shared launcher, including New, Clone, Replace with and Restart with: search refusals,
request errors, directory and checkout errors, template failures, and a failed save of the YOLO “don't ask again”
answer. Bordered launch alerts keep their icon and border. Loading and ordinary typing hints stay plain. The permission
escalation banner, its outline pulse and YOLO danger markings stay red.

A merely displayed default or a remembered permission does not trigger a banner when choosing the first agent. Other
permission modes and effort still need an agent first. The Command tab offers no permission rows. Existing permission
reconciliation and Undo are reused.

The product and implementation specifications and the Start a session documentation were updated. The original TODO
entry was removed. The two PRs remain drafts for the landing monitor.

## Open questions and possible follow-ups

None required for this plan. There are no omitted acceptance criteria or unanswered design decisions.

## The PRs

- [#2987: yellow launcher rejections](https://github.com/scode/farhelm/pull/2987/changes).
- [#2989: permissions before choosing an agent](https://github.com/scode/farhelm/pull/2989/changes).

## Checks run, reused and skipped

Runtime checks used the test recorder, with pinned nextest for Rust and pinned tmux for browser stacks. Counts below are
case executions and overlap between runs.

- `cargo nextest run -p farhelm-ui -E 'test(launch_composer::) | test(create_form::)'`: PR 1 passed 167 selected cases,
  run `307eccd3-1097-44c7-8051-2d00bef46388`; the combined change passed 168, run
  `7ed04039-468f-46b9-9f71-eeaf6af1d05f`. The other 675 cases were filtered out in each run. No runtime substrate skips
  occurred.
- Selected `composer-word-search`, `yolo-guard` and `github-checkout-composer` browser cases passed 18/18 on Chromium
  and WebKit, run `9c329886-565c-4d90-8859-dedfad3eb794`. These cover both themes, ordinary hints, launch alerts, a
  failed YOLO preference save and a failed repository scan.
- The same applicable selection with `FARHELM_E2E_SUITE_TERMINALS=host` passed 16/16, run
  `aabcfe07-4c1f-4943-b0ae-35249b9a1556`. That suite's inventory excludes `yolo-guard`; the ordinary run supplies that
  coverage.
- `npx playwright test 'permission-switch\.spec\.ts|composer-word-search\.spec\.ts'` passed 38/38, 19 per engine, run
  `7cb15ede-b183-4719-a82a-0822ec20dc05`. It covers the combined change, including first-agent escalation, explicit
  versus remembered choices, Undo, existing agent/model/template switches and banner lifetime behavior. No browser
  failures, skips or global errors occurred. `permission-switch` is outside the host-terminal inventory; these cases
  exercise launcher choices rather than a terminal.
- `cargo build -p farhelm`, the web build with `dx build --package farhelm-ui --platform web --release`,
  `cargo clippy -p farhelm-ui --all-targets -- -D warnings`, `cargo fmt -p farhelm-ui -- --check`, configured Markdown
  formatting checks, `python3 releasing/check-changelog.py format`, and the isolated-interpreter
  `python -B scripts/check-test-sleeps.py` passed. The delay checker inspected 403 calls, with no missing rationale.

The final browser run covered `9dee4e209b809faa50ed213712ecc415e4d53a8b`. The stack was then rebased onto main's
unrelated TODO addition. Both PRs have an empty interdiff against their reviewed versions; executable sources and
selected tests are unchanged. Final heads are `efd884e24d6e679158688e703514bc2c06d9e7a5` and
`5ced6faa356ad643305f3f889aff910df8322c1d`, so the recorded checks remain applicable. The backend build is reused
because this plan changes no backend code.

Full workspace, full browser, desktop, installer and provisioning suites were skipped: the identified risks are launcher
search, permission transitions and presentation, covered above. The incidental docs edit changes prose without moving
pages or changing links, so no website build or preview was needed. CI is manual and was not dispatched.

## Review gate outcome

Each PR passed its required independent gpt-6.1-sol/high review. PR 1's reviewer found two missed warning paths, the
failed YOLO preference save and repository discovery failures; both were corrected and the review passed. PR 2 passed
without findings. Commit messages and PR text passed fresh-context cold reads. The rebase preserved both reviewed
patches without changes.
