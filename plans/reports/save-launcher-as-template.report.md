## What this was about

New and Clone can now save the launcher's current setup as a launch template without retyping it in Templates. The
inline panel offers the active tab's choices as a checklist, with deliberate choices already checked. Saving keeps
exactly the checked choices and the active launch kind, closes the launcher, and opens the new template in the Templates
editor. Replace with has no save action.

## Things you should know

The checklist stays fixed while it is open. Agent, model and effort choices copied by Clone count as deliberate; a
remembered approvals or trust default, an inherited Clone host, and an inherited or default folder start unchecked. Host
and folder choices made by hand are checked. A repository destination is checked whenever present. Unsupported approvals
or trust controls are omitted. Command resume off is omitted. The host uses the launcher's readable name in the
checklist while the saved template keeps its installation identity.

A taken name is refused with the existing Templates message. The implementation retains the existing list-then-save API
behavior; it adds no atomic protection against two clients creating the same name simultaneously. Enter in the panel's
name field saves without launching a session; Escape closes only the panel. After a successful save, the launcher draft
is gone.

Validation reproduced an existing WebKit fixture failure before terminal interaction: a healthy session's initial row
render raced the helper's five-second deadline. The helper now uses the sidebar fixture's existing 20-second readiness
budget and reports whether the session still exists if readiness fails. The two failed runs remain retained, and
FLAKES.md records the evidence. CPU scheduling pressure was not established as the underlying cause.

## Open questions and possible follow-ups

No decision is required to use this feature. The docs explain saving from the launcher; screenshot publication is
deferred to a later refresh, as the plan permits. The existing concurrent same-name save behavior is unchanged.

## The PRs

- [#1717](https://github.com/scode/farhelm/pull/1717/changes) — save launcher setups as templates; feature,
  specifications, user documentation and the reproduced fixture correction in one coherent draft PR.

## Checks run, reused and skipped

Run for the final corrections: UI all-target Clippy, desktop UI compilation, release web build, the isolated
source-delay checker (269 delays, zero missing reasons), FLAKES.md formatting, and the website build with frozen
dependencies and internal-link validation. The website build was repeated after rebasing because main changed its
release-notes generator. The readable host correction received a separate documentation pass and targeted compilation
and visual checks.

Recorded checks:

- UI/protocol unit run `1416f769-6cc8-4eb5-acdc-0863f571defa`: 569 passed, zero skipped; no runtime skip messages.
  Reused from content-identical source at `3293195e`: later changes affect host-label presentation and a browser
  fixture, leaving the tested mapping and protocol behavior unchanged.
- Three-spec Chromium/WebKit run `0bcdfedf-2839-4732-b2c2-8accc96344a6`: 37 passed, one existing WebKit readiness
  failure. All launcher-save, name-refusal, keyboard and editor-handoff cases passed both engines. Reused the successful
  templates and launcher-tabs cases because their product behavior is unchanged. This run is retained as a failure, not
  described as a clean pass.
- Exact unchanged WebKit case, run `fb09f027-72c6-4384-b643-6573f1c895dd`: two passes and one reproduction of that
  readiness failure. Its attachments were retained before rerunning.
- Corrected exact WebKit case, run `98ba4b12-ce93-414d-8911-c4d759db83a0`: three passes in three repetitions, one worker
  and zero retries.
- Strict tooltip spec, run `03a479d7-8690-4965-94c5-a15b94b970ab`: four passes on both engines, zero skips, complete
  reports and cleanup. Its source record includes obsolete generated website Markdown from an earlier owned build; those
  unused outputs were removed. They do not affect the built UI or these tests. Reused after the host-label-only
  presentation correction; every control and tooltip remains the same.
- Final real-stack panel capture `694128ce-4c99-40a1-a76d-bd78e2dd487a`: four passes at widths 1280 and 390 in
  Chromium/WebKit, with readable-host and overflow assertions. All four images inspected; no clipped panel values or
  save controls. Tested clean source `9ca2763b`; subsequent feature edits only add this PR number to the flake history;
  the final rebase also preserves an unrelated TODO addition on main.

Also reused successful workspace all-target Clippy, Rust formatting, changed-file dprint, changelog-format lint and CLI
build from the earlier feature validation. UI Clippy was repeated after each code correction, covering the only Rust
crate changed since the workspace lint. Main's intervening changes were inspected: the New shortcut and shared
template-kind inference were incorporated before feature validation; later release-note screenshot tooling, queue
approvals and an unrelated row-layout TODO addition do not alter launcher contracts or runtime code.

Skipped additional workspace, desktop runtime, installer, provisioning and full browser batteries: the changes introduce
no new integration risk beyond the unit mapping, compiled UI, selected real-stack browser cases and four captured
layouts. No hosted CI, screenshot publication or website deployment was requested.

## Review gate's outcome

The required sole reviewer ran on gpt-6.1-sol at high effort with the full acceptance criteria and verbatim
test-authoring checklist. Two initial findings were fixed: copied unsupported approvals/trust choices and the effective
name of a GitHub Clone. Follow-ups found no remaining defects, including in the row-readiness correction and readable
host-label snapshot. The executor inspected the fixes and review artifacts and ran the validation above. Commit/PR
wording passed a separate gpt-6.1-sol medium cold read; the PR body is empty. Native model-reporting and usage counters
were unavailable; private galaxy session evidence records that gap. Implementation and investigation stayed with the
executor.
