## What this was about

Keyboard users of the Mac app and Safari could not reliably scroll through a long host setup or uninstall plan before
approving it. Those dialogs keep their answers outside the scrolling plan, whose text offers nothing to focus. The trash
list has the same problem when all its actions are disabled. WebKit does not automatically put these scroll areas in the
tab order.

The maintainer approved named tab stops and the existing focus style for these areas, plus a standing specification rule
that every GUI scroll region be reachable and scrollable by keyboard. Both fixes stay within those markup and
focus-style limits.

## Things you should know

Tab now reaches each host plan and the trash list, and PageDown scrolls their contents while the surrounding controls
stay in place. Initial confirming focus and authorization behavior are unchanged. The trash region remains reachable
while measurement leaves every action inside it disabled.

The browser checks run the shipped web renderer on Linux with Chromium and WebKit. They establish keyboard access in
those engines; the Mac desktop shell was not exercised. No service or installation used by the operator was changed.

## Open questions and possible follow-ups

SSH connection help has another scroll area without its own tab stop. While loading, reporting an error, or showing no
available commands, it can contain only text while its close buttons remain outside it. This was found by inspecting its
markup and scroll styling, without a separate browser reproduction. Its keyboard access is a follow-up; the maintainer
limited this plan to host plans and trash. The specification's broader rule does not mean every existing scroll area was
audited or repaired here.

## The PRs

- [#2190: keyboard access to host setup and uninstall plans](https://github.com/scode/farhelm/pull/2190/changes).
- [#2191: keyboard access to the trash list](https://github.com/scode/farhelm/pull/2191/changes).

## Checks run, reused and skipped

- Host setup layout tests on Chromium and WebKit: four passed in recorder run `cbca5cb3-6087-4114-862c-7d6178f59596`.
  That first selection also included uninstall, whose two cases explicitly skipped because the isolated container lacked
  passwordless self-SSH. It supplies no uninstall evidence. Selection:
  `npx playwright test 'provisioning.spec.ts|terminal-multihost.spec.ts' --grep 'host-setup-layout|uninstall-remote-host: the plan'`.
- After configuring loopback SSH inside that owned container, the narrow real-fleet uninstall test passed in both
  engines: two passed, zero skipped, run `4a3150bc-7dba-4cc9-9d1d-ebe0b2f442af`. The helm, supervisor, SSH connection,
  and plan were real; the existing provisioning backend simulates host installation/removal actions. Selection:
  `npx playwright test terminal-multihost.spec.ts --grep 'uninstall-remote-host: the plan'`.
- The complete trash dialog spec passed 20 cases, ten per engine, with zero skips or failures in run
  `9829a85c-3483-48e3-a01e-a1c13bbbd42b`. This includes keyboard scrolling with every list action disabled, existing
  single and bulk cleanup, progress, confirmation dismissal and focus restoration. All three recorder reports were
  complete, and their retained output was untruncated and read. Selection: `npx playwright test trash-dialog.spec.ts`.
- `cargo clippy --offline --all-targets -- -D warnings` passed for each PR, and the required CLI/fixture and web builds
  passed. `cargo fmt --all -- --check`, `dprint check`, the isolated test-sleep checker (302 calls, no missing
  rationale), `python3 scripts/check-agents-md.py`, and `python3 releasing/check-changelog.py format` passed. Builds:
  `cargo build --offline -p farhelm -p farhelm-fixtures` and `dx build --package farhelm-ui --platform web --release`.
- Host-plan browser evidence covers the source in PR #2190 at `db2519645627` and is reused at the final tip: the second
  PR changes only trash markup, its focus offset, its browser spec and bookkeeping, leaving the host plans and shared
  keyboard assertion helper unchanged. The latest fetched main still matches the stack base, so no rebase changed the
  tested behavior.
- Rust runtime, desktop-shell, JavaScript-unit, installer, full browser, and website checks were skipped: these changes
  add no backend or JavaScript logic, desktop-specific branch, installation behavior, or website content. Focused
  browser cases exercise the affected native keyboard behavior directly. No CI run was requested or triggered.

## The review gate's outcome

Each PR passed a separate fresh-context gpt-6.1-sol high source review covering correctness, design, idioms, the full
outcome and complexity gate, and the full test-authoring checklist. Neither review found a defect. The executor checked
the findings against the changes and owns the runtime evidence above; the reviewers ran no tests. Fresh gpt-6.1-sol
medium cold readers checked the commit and PR wording. Contaminated blind reads were excluded and replaced with fresh
readers. No implementation work was delegated.
