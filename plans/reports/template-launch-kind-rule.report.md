# Shared rule for a template's launcher tab

## What this was about

Older launch templates do not record which launcher tab they switch to. Farhelm infers it when an agent creates a
kindless template and when the Templates editor opens an older one. Those rules agreed, but separate copies of the field
lists could drift as launcher choices evolve and make the same choices select different tabs. The maintainer asked for
one shared rule, with existing behavior preserved.

## Things you should know

The helm and Templates editor now use the same rule. Command, YOLO and resume-command choices imply the command tab;
otherwise an agent type, model, effort, permissions or workspace trust implies the agent tab. Host, destination and
session name alone imply neither. A false value or an explicit reset still counts as a choice.

An agent type beside command choices still declares that command's agent. It does not make the template invalid by
itself; agent-only choices such as model or effort beside command choices remain refused when an agent writes a
template. Explicitly recorded tabs remain authoritative. Stored templates are not migrated, and opening an older one
still changes only its unsaved editor draft. Applying templates is unchanged.

This is a maintenance refactor with no user-visible behavior change. The completed TODO entry is removed; no changelog
fragment was needed.

## Open questions and possible follow-ups

None. The requested scope is complete; no product decision or follow-up is required.

## The PRs

- [#1715: share the rule for a template's launcher tab](https://github.com/scode/farhelm/pull/1715/changes) — one draft
  PR, not marked ready or merged by the executor.

## Checks run, reused and skipped

- `cargo fmt --all -- --check`: passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `cargo clippy -p farhelm --bins -- -D warnings`: passed, including the shipped binary's configuration.
- `dprint check TODO.md`: passed.
- `python -B scripts/check-test-sleeps.py`, using the isolated pinned parser environment: passed; 269 delays inspected,
  zero missing reasons.
- Recorded nextest run `55b7461f-b19e-44b9-8c73-02fb09f3ace9`, pinned nextest and tmux, four slots and zero retries:
  `cargo nextest run -p farhelm-proto -p farhelm-helm -p farhelm-ui --lib -E
  '(package(farhelm-proto) and test(launcher::)) or (package(farhelm-helm) and
  test(agent_requests::tests::template)) or (package(farhelm-ui) and test(list::templates::tests::))'`:
  all 15 selected tests passed, with complete JUnit evidence and no runtime skip messages. Includes the new
  field-by-field table, unchanged helm create/edit and mixed-choice refusal checks, and unchanged Templates editor
  tests. The 1560 other cases were selected out.
- These checks cover the working diff on base `2ac13c6e` and are reused for pushed commit
  `a857fc1bc50f42d9aa977b674d5d7cdb26ac2fc0`. Careful rebase inspected every intervening commit: only queue claims and
  another plan's report landed. The product code and this plan's diff were preserved, with no conflicts. No
  prior-session runs were reused.
- Browser, JavaScript, desktop runtime, installer and full-workspace runtime suites skipped: this changes shared pure
  inference and its existing callers without changing behavior; the selected tests cover the affected contracts. No
  executable documentation or new doctest was added, so no doctest run was needed.

## Review gate's outcome

The required fresh-context `gpt-6.1-sol` high-effort reviewer found no issues after checking the diff, authoritative
specifications, unchanged callers and tests, and the complete test-authoring charter. The executor read the findings,
verified the contract preservation, and made a separate documentation pass over every touched file. The independent
commit and PR wording cold read passed. Implementation stayed local; no workhorse was used.

### Landing

Landed on 2026-10-08 (UTC) as #1715 (one shared rule for a template's launcher tab), one squash commit on main. The plan
waited about nine hours after delivery because the monitor was down during a host reboot.

#### What else was on main

Between the commit the change was built on and the landing, the new-button-shortcut-hint plan landed (#1714, the Mac
desktop app's New button hint). The two share no code; each removed a different TODO entry, and the rebase applied
without conflict. Otherwise main gained only the planning queue's own bookkeeping.

#### Review before merging

A separate reviewer that had not worked on either plan checked both before anything merged and found nothing that
breaks. It compared the new shared rule field by field with the two copies it replaces, the helm's and the Templates
dialog's: the same command fields, the same agent fields and the same order, so nothing a template does changes. The
helm's refusal of templates that mix agent-only and command choices behaves as before, and nothing else called the old
copies.

The report's checks did not build the web app, which now uses the shared rule from the protocol crate. The landing did,
below.

#### Checks

- Run now, after the rebase: `cargo check -p farhelm-ui --features web --target wasm32-unknown-unknown` (the web app's
  build), `cargo fmt --all -- --check` and `dprint check TODO.md`, all clean.
- Reused: the report's other checks. The rebase brought in only #1714, which touches nothing this change uses.
- Skipped: running the tests again, for the same reason.

Nothing in the report above was made untrue by the landing.
