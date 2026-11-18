## What this was about

The launcher and template editor already shared their controls for where a session runs and whether it opens a folder or
creates a managed checkout. Those controls lived inside the launcher dialog's code. They now have a documented home of
their own so changes to either screen can account for the other.

If the last host that can run containers disappears, both screens keep the selected New container choice visible. The
editor used to guard against an extra press on that choice. In the launcher, that press could clear the message
explaining why a launch was refused. The shared picker now ignores the repeat press when it has no host to offer.

## Things you should know

- The template editor keeps the selected container recipe and host, and the Host selector stays hidden while there are
  no eligible hosts. A launch refusal stays visible when the launcher receives the same repeat press. That visible
  effect warranted a fix and a changelog fragment.
- The launcher still checks whether its hosts are ready and shows its build and placement notes.

## Open questions and possible follow-ups

None for this round.

## Pull requests

- Preserve the refusal message and cover the template editor's retained choice.
  [#4130](https://github.com/scode/farhelm/pull/4130/changes)
- Move the shared destination controls, document their ownership and remove the two completed TODO entries.
  [#4133](https://github.com/scode/farhelm/pull/4133/changes)

## Checks

- All 129 selected Linux tests passed with no runtime skips in recorder run `af92e838-2d5f-4d31-8f3e-e0ed208ac099`. The
  recorded command was
  `cargo nextest run -p farhelm-gui --lib -E 'test(/^launcher::environment_/) | test(/^launcher::checkout_tests::/) | test(/^templates::editor_tests::/)'`.
  It covered the launcher refusal and template editor repeat-press regressions through the shared controls in #4133.
- `cargo clippy --locked -p farhelm-gui --all-targets -- -D warnings` passed on Linux after an interrupted first
  attempt.
- `cargo fmt --all -- --check`, `dprint check` on the changed Markdown, `python -B scripts/check-test-sleeps.py` in the
  isolated checker environment, `python3 releasing/check-changelog.py format`, and `git diff --check` passed.
- All 51 tests in the preliminary launcher environment-view and template-editor selection passed with no runtime skips
  in recorder run `b5bbb717-65e0-42f9-8111-0b1eacfd551a`. The recorded command was
  `cargo nextest run -p farhelm-gui --lib -E 'test(/^templates::editor_tests::/) | test(/^launcher::environment_view_tests::/)'`.
  The 129-case run supersedes it with the stronger editor test, the launcher refusal assertion and the controls in their
  new home.
- Skipped the full workspace suite and screenshot capture. The focused tests exercise both affected screens, and the
  controls draw the same rows.

## Review gate

Each PR passed independent code review. The first review identified the launcher refusal side effect and a weak
assertion in the editor test. The patch preserves the refusal and the editor test confirms that the repeat press reaches
the already-selected choice. The second confirmed the control definitions moved intact. The ownership docs now identify
where host readiness comes from, and the parity docs distinguish quick launch's preparation from its fallback dialog and
preserve the existing behavior notes. All findings were addressed. Both commit messages passed independent cold reads.
