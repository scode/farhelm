## What this was about

The November 18 triage settled nine reports about the terminal, credential entry, template shortcuts and disposable
environments. The maintainer chose to discard these nine reports for the reasons recorded in the triage ledger.

## Things you should know

Each PR removes one report from the active review queue and moves the corresponding decision to its archive. The
archived decisions preserve the assessment, the maintainer's choice and a link to the PR that carried it out. These are
bookkeeping changes.

The active environment-history report remains queued. Both separate reports about confirming a tab's terminal host
during session deletion remain queued. Those three reports were outside the duplicate discards approved here.

## Open questions and possible follow-ups

Some links in the historical review collection still point to the removed queue entries. The original events are still
in the historical collection. Use the archived decisions to find a disposition and its PR. Editing the historical
summaries would change the collected record.

## Pull requests

- Copying a terminal selection can include characters the program concealed. The maintainer accepted that behavior for
  selection, which copies the text stored in the selected cells.
  [#4157](https://github.com/scode/farhelm/pull/4157/changes)
- Operating-system text services can request the contents of a masked field. The maintainer chose to focus this round of
  credential fixes on logs and the app's visible surfaces. [#4158](https://github.com/scode/farhelm/pull/4158/changes)
- Saving a Templates number-key change while the window has an older set of bindings can overwrite an intervening edit
  to another key. Triage accepted last-save-wins for these bindings.
  [#4161](https://github.com/scode/farhelm/pull/4161/changes)
- The report warned that cleanup could choose a different tmux target after the original vanished. Farhelm now cleans up
  through terminal hosts and verifies the process it ends. [#4162](https://github.com/scode/farhelm/pull/4162/changes)
- Removing tests for the old terminal tabs looked like a gap in coverage. Their removal shipped in the same release as
  retirement of the tabs those tests covered. [#4164](https://github.com/scode/farhelm/pull/4164/changes)
- The report raised the possibility that removing a browser clipboard classifier exposed clipboard reads. The current
  terminal emulator drops programs' clipboard read requests. [#4165](https://github.com/scode/farhelm/pull/4165/changes)
- An environment created by an agent can change the launcher's remembered choices and push recent setups out of its
  history. The active environment-history report covers that behavior.
  [#4166](https://github.com/scode/farhelm/pull/4166/changes)
- The earlier Retry label suggested a connection attempt even when the action continued a deletion. The action now says
  “Finish the deletion,” and the helm accepts it after an already confirmed deletion failed.
  [#4167](https://github.com/scode/farhelm/pull/4167/changes)
- The report asked whether deleting a session could leave a terminal tab's host process running after its socket
  disappeared. The investigation found no shipped case, and two active reports still track process confirmation in
  related paths. [#4168](https://github.com/scode/farhelm/pull/4168/changes)

## Checks

- `dprint check TRIAGE_OUTCOMES.md TRIAGE_OUTCOMES_ARCHIVE.md review_feedback_queue/INDEX.md` and `git diff --check`
  passed for each final draft and the combined stack. Every patch removes exactly one finding and its index item, and
  preserves the triage entry apart from the Execution field. `python3 scripts/check-agents-md.py` passed with the
  pre-existing warning about an archived citation. The checked stack tip is `7d444b63`.
- Main advanced to `ffe12567` with two other plan deliveries. Those commits touch the plans queue and reports. The
  checked triage files still match the stack's base.
- Runtime tests and hosted CI were skipped because the drafts touch only the review queue and triage ledger.

## Review gate

Every draft received a fresh independent patch review and a title cold read. The patch reviews confirmed the exact moves
and preservation of related reports. I kept the prescribed scope where review found references to the removed entries in
historical summaries. The final PR numbers, heads, bases and draft states were checked against GitHub.
