## What this was about

A copied folder or session name could look unchanged after an edit in Clone while the launch used a different path or
name. Farhelm showed invisible characters as text such as `<U+200B>` and then stored that visible text on the first
keystroke. Command fields and Custom flags had the same problem, and Bash could read the resulting angle brackets as
redirection. The affected fields now keep invisible characters through edits, and the recent-folder filter uses the same
underlying path as the launch.

Restart with and Replace with also said “then launch” when a suspended or stopped host needed to be resumed in Hosts.
The advice now names the action the user was trying to take.

## Things you should know

As agreed when planning this fix, an exact spelling typed into an affected field stands for its invisible character. If
a copied value contains literal text such as `<U+200B>`, editing the field converts it to the character even when it
still looks the same.

Restart with still spells out a stored command or resume command containing invisible characters and makes that field
read-only. An untouched command remains available for a restart that changes another setting. Clear empties that field
and makes it editable, even if the old command is then typed back in.

## Open questions and possible follow-ups

Pasting a raw invisible character in the middle of a field can move the caret to the end and clear that field's Undo
history when Farhelm replaces its display with the spelling. An edit beside an existing spelling keeps the editor's
normal history. Preserving caret and Undo through live reformatting would need input-widget support that covers paste
and input methods together. That work is deferred.

## PRs

- [#4131](https://github.com/scode/farhelm/pull/4131/changes) keeps invisible characters through launcher edits.
- [#4132](https://github.com/scode/farhelm/pull/4132/changes) names the action to retry after resuming a host.

## Checks

- Ran locally on Linux. The recorded `cargo nextest run --locked -p farhelm-gui --lib -E <focused selection>` passed all
  146 selected tests at `fcbbc0056a9e`. Run `ae1a08f4-7d67-4c93-b21a-53e8aca37f1b` covered launcher editing, templates,
  quick launch, Resume wording and session-list environment actions. No selected test reported a runtime skip.
- `cargo clippy --locked -j 6 -p farhelm-gui --all-targets -- -D warnings`, `cargo fmt --all -- --check`, `dprint check`
  on changed Markdown, `python3 releasing/check-changelog.py format`, `python -B scripts/check-test-sleeps.py`, and
  `git diff --check` passed. `python3 scripts/check-agents-md.py` passed with an existing archived citation warning.
- In run `2849989c-d129-47bd-9882-6eb9a61da57b`, the Clone test clicked Name while the recent-folder dropdown covered
  it. The earlier interrupted run `847f68dd-1e1f-4aa9-b04a-a2f7a5327570` recorded the same failure. The corrected test
  passed by itself in run `58651975-6c21-42bd-8426-bd9e3a2c4e63` and in the final selection.
- Reused no runtime runs. Skipped screenshot goldens because their scenes do not show the changed inputs or advice. The
  focused selection covers the changed GUI behavior, so the wider workspace and desktop smoke suites were skipped.

## Review gate

Both PRs cleared fresh-context GPT-6.2 Sol high-effort review and a cold read of the commit and PR text. Review caught
the recent-folder filter reading the displayed spelling and strengthened the test for editing after Clear. The paste
behavior is deferred as described above. This report received a separate cold read before delivery.

### Landing

#### What landed with it

It landed second in a round of five plans, right after submenus-open-on-hover and before
retire-old-terminal-announcement, quick-switcher-shot-paths and update-survival-wording. Since its base, main gained the
launcher's shared destination controls (#4130, #4133), which moved launcher code next to the code this plan changes.
Those controls have no text fields, so the new rule for spelled-out characters misses nothing. Main also gained docs
screenshot and README hero changes, a docs refresh and bookkeeping, none of which touch the launcher's fields. Another
plan in the round also edits SPEC.md, in a different section.

At your request this landing was reviewed with extra skepticism, both by me and by an independent reviewer. The fix is
correct and no bigger than the plan asked:

- Only the app's own spelling of an invisible character, such as `<U+200B>`, turns back into that character. Lowercase
  or zero-padded spellings, and the code of a visible character (`<U+200b>`, `<U+0200B>`, `<U+0041>`), stay as typed.
- One rule per field decides both how it shows its value and how an edit is read back.
- When a launch is refused because its host is suspended or stopped, the message now ends with the current mode's action
  ("…then restart" in Restart with, "…then replace" in Replace with), not always "…then launch".
- The four tests the plan named were split, renamed or simplified as asked.

#### Fixes made while landing

- **A test half was deleted rather than updated.** The Clone test used to type an invisible character into the copied
  name and check that the field spells it out at once. The plan changed what the result should be, but the executor
  deleted that half instead of updating it. Nothing then checked that a typed or pasted invisible character is spelled
  out immediately, or that a spelling typed by hand becomes the character, which you accepted on the planning page. The
  half is back with the new expectation, plus a typed spelling.
- **The text fields' code comment lost its map.** It had listed which launcher fields are read-only until Clear and
  which are editable, with the reasons and the old app's specs. The plan cut it to two short paragraphs. It is rewritten
  to say what is true now: three kinds of field, which fields are which, and the typed-spelling rule.
- **A parity note that no longer described a difference.** `docs/gpui/parity/launcher.md` said that in Restart with, a
  stored command you retype becomes read-only again, which the old app did not do. This plan removed that difference, so
  the note now names the one that remains: a hidden character typed after Clear is spelled out.
- **Two code comments lost their constraints.** One dropped that a stopped environment is never called suspended
  (SPEC.md, "Container sessions"). The other, on a test helper now shared between test files, no longer said that the
  helper leaves the field texts stale.

#### Things you should know that the report does not say

- **Replace with is not covered.** It also copies the source's folder and title into editable fields, and those fields
  show invisible characters raw, as before. The plan was about Clone, so this is unchanged. If you want Replace with to
  match, it is a small follow-up.

#### Checks

This round landed five plans together, so these checks cover all five on one tree: the five stacks one after another on
main. The plans change the desktop app's menus, launcher and quick switcher, the helm's release announcement table,
SPEC.md and documents. The landing ran:

- the lints and source checks;
- the whole test suites of the desktop app's GUI crate and of the helm;
- the app's end-to-end smoke;
- the reference screenshots (goldens);
- a hands-on run of the app, with screenshots of each submenu step (kept out of the repository).

Builds ran locally. The test suites and the smoke ran on boat sandboxes (cloud VMs), from binaries built locally.

On the plans as delivered (main had gained only bookkeeping, the docs screenshot tooling, a docs refresh and the
launcher's shared destination controls since their bases):

- `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py`, `dprint check`,
  `python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py`,
  `cargo clippy --all-targets -- -D warnings` and `cargo clippy -p farhelm --bins -- -D warnings`: passed.
- `cargo nextest run -p farhelm-gui -p farhelm-helm`, from a locally built archive, split in three across boats: 1655,
  1603 and 1597 passed, 4855 in all (`1925a98b-8cce-47e4-ad2a-42f7d3d355ce`, `11bd4bb4-ae5f-45e9-a5a9-9be345c3a126`,
  `8a6fa767-830d-4071-a283-f25dea7661a1`). Three helm tests skipped at runtime because the archive does not carry a full
  `farhelm` binary: provisioning over SSH and over the local transport, and the uninstall stop. None touches these
  plans; that coverage is missing from this run.
- The end-to-end smoke (`python3 -B scripts/gui-e2e-smoke.py`), on a boat: 48 of its 49 steps passed and the clipboard
  paste step was skipped, as always on Linux (`9efe756f-4b33-4fe0-bd9a-02bffc4b3bdc`).
- `python3 scripts/check-gui-goldens.py`, locally: passed (`061c16ed-d3c3-4664-85d3-1d3340c34326`).

On the fixed tree (comments, SPEC.md, documents and two tests changed):

- `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py`, `dprint check`,
  `python3 scripts/check-agents-md.py` and `cargo clippy --locked -p farhelm-gui --all-targets -- -D warnings`: passed.
- `cargo nextest run -p farhelm-gui --lib` for the session menu, submenu, Clone entry and quick switcher tests, locally:
  108 passed (`56e7255b-5924-40ff-bee9-03f8cf0bbe7f`), the two changed tests included.

The boat suites, the smoke and the goldens were reused for the fixed tree: after them only comments, SPEC.md, documents
and two tests changed, which the focused run above covers.

Skipped:

- The rest of the Rust workspace and the desktop process's tests: the plans stay in the GUI crate and the helm.
- The Linux app lifecycle: startup and quit are untouched.
- The Mac-only checks: no Mac-specific code changed. The submenu behavior was checked on Linux only.
