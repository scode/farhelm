# Clone into a fresh checkout

## What this was about

Clone previously kept a session in its source folder, even when that folder belonged to a managed GitHub checkout. The
plan changes Clone in the helm to start a fresh checkout of the same repository. This applies to both the session that
created the checkout and sessions using any folder within it; checkout membership takes precedence over launch history.

## Things you should know

An untouched name starts at `<source title>-clone`, then searches for the lowest available suffix from `-clone-2`
through `-clone-50`. Choosing another host installation or repository restarts that search. Explicitly edited names keep
their conflict, rather than silently changing. Choosing an existing folder restores the source folder and copied title.
An empty source title, or a different repository, uses the existing unnamed-checkout naming rule.

If another process takes the proposed folder before Launch, the form proposes the next free name and waits for another
Launch click. If the launch reply is lost, retrying unchanged reconciles the original request and allocation; an
occupied preview alone must not change its name. There is one edge case: changing the launch while that reply is
unresolved can advance the suggested name, and undoing the edit does not restore the original retry. Launch can then
create a second session. Reselecting the repository restarts the search and restores the opportunity to reconcile the
original name. This is documented, and the existing retry contract only applies when the launch intent still matches.

The 50-candidate limit bounds the search if a peer keeps reporting conflicts. At the limit the form shows the conflict
and waits for the user to change the name or destination. A rare installation-identity race can be classified as an
occupied name and reach that limit; stale-connection refusals are excluded. Replace, Replace with and
`farhelm agent clone` keep their existing behavior. The change is in the launcher UI; server and protocol behavior stay
the same. The docs website page was built locally; it was not deployed.

## Open questions and possible follow-ups

None needed to complete the plan. The unresolved-reply/edit/undo edge case above is a possible follow-up if restoring
the original suggested name after an intent edit becomes a product requirement. Freezing every retained attempt instead
would also prevent the normal suffix search after a deliberate launch change, so this plan keeps the current intent
matching rule.

## PRs

- [#1684 — Clone GitHub checkout sessions into fresh checkouts](https://github.com/scode/farhelm/pull/1684/changes)

## Checks run, reused and skipped

Final source checks passed: `cargo fmt --all -- --check`, targeted `dprint check`, the isolated
`python -B scripts/check-test-sleeps.py` (269 delays, zero unannotated), and
`python3 releasing/check-changelog.py format`. These cover formatting, test-delay rationale and the new fragment.

Four focused UI nextest tests passed in run `fc702283-6177-469d-89b3-f92af9d09733`, covering repository prefill,
copied-title behavior, default-name search/reset/cap, and preview error classification. Two focused tests passed in run
`3ff66a69-942c-433d-a7dc-2b19d712b997`, covering the retained-request guard and existing intent matching. These used the
plan's working tree on base `07aa85e5`, before later comments and the browser completion attribute. Their pure-policy
coverage still applies; the later mounted UI behavior was checked in the browser run.

`cargo clippy -p farhelm-ui --all-targets -- -D warnings`, `cargo build`, and the release web UI build passed after the
completion attribute and its borrow correction. The website frozen install/build passed (31 pages). The final changes
since those runs were comments, an implementation-spec caveat, and test cleanup if a deliberately refused create
unexpectedly succeeds, so no build or runtime repeat was needed.

Recorded Playwright selection
`github-checkouts.spec.ts -g 'clone defaults|clone lost race|clone retains its name|borrowers
retain|replace with into a fresh'`
passed all ten cases, five each on Chromium and WebKit, one worker and zero retries: run
`d3662deb-dcd6-478f-86c0-318353bacb10`. It covers fresh Clone of origins and subdirectory borrowers, repeated suffixes,
explicit conflicts, folder restoration, different repositories, lost-race click boundaries, lost-success reconciliation,
borrower lifetime and unchanged Replace-with behavior. The recorder reports zero skips, flakes or failures. This tested
the final behavior on base `07aa85e5`; the only subsequent test edit adds cleanup to an unexpected-success failure path.

No full Rust or browser suite, desktop runtime, installer, provisioning or release checks ran: the changes are confined
to the shared launcher and preview error interpretation, with focused policy tests and both browser engines covering the
concrete integration risks. Main was fetched before the PR and had not changed since the plan base, so no rebase or
interaction checks were needed.

## Review gate outcome

Claude Opus 5.5 high and GPT-6 Astra high independently reviewed correctness, design, language idiom and the full test
authoring checklist. The retained-request bug and browser completion-oracle gap were fixed and rechecked. Both final
reviews found no acceptance-criteria failures or unnecessary complexity. Final Opus documentation and cleanup nits were
applied; its launch-edit edge case is documented above. The suggestion to trace two deterministic policy tests was
declined because they have no process or lifecycle evidence to preserve. The implementation was done locally; only the
required reviews and process cold reads were delegated. Reviewers inspected source and did not run the tests themselves.

### Landing

Landed on 2026-10-06 (UTC) as #1684 (Clone of a session in a managed GitHub checkout now offers a fresh checkout of the
same repository), one squash commit on main.

#### What else was on main

Between the commit the change was built on and the landing, the mac-release-test plan landed (#1682, a release-test
override for the Mac app's update check, and #1683, the Mac VM test documents). A separate reviewer that had not worked
on either plan read both against each other before anything merged: they share no code, edit different paragraphs of
SPEC_impl.md and remove different TODO entries, and the rebase applied without conflict. Otherwise main gained only the
planning queue's own bookkeeping.

#### Review before merging

The same reviewer found nothing outside the PR that the change breaks. It confirmed:

- The change only applies when the source session belongs to a GitHub checkout; Replace and Replace with are unchanged,
  and Clone from the session header uses the same code as Clone from the session's row.
- No browser test outside the two GitHub-checkout spec files creates a session in a checkout, so the clone, header,
  replace, Restart with and sidebar tests are unaffected. The one test on main that assumed the old behavior was updated
  in the PR.
- None of the docs screenshot, README image or demo video scripts clicks Clone.
- `farhelm agent clone` is unchanged: the PR touches nothing in the command line, the helm or the supervisor, and that
  command does not go through the app's launcher. The changes in the protocol crate are documentation comments only.
- No new controls were added, so the hover-text coverage test is not involved.

It also found two passages outside the PR that are now out of date, which the landing did not change and nothing else
tracks: `docs/github-checkouts.md` still says an ordinary Clone reuses the source session's folder, and the website's
"Stop, restart and resume" page says Clone opens the launcher the same way Replace with does, which is no longer true
for sessions in a GitHub checkout.

#### Checks

- Run now, after the rebase: `dprint check` on SPEC_impl.md and TODO.md, clean.
- Reused: the report's checks. The rebase brought in only the mac-release-test plan, which touches nothing this change
  uses.
- Skipped: running the Rust and browser tests again, for the same reason.

Nothing in the report above was made untrue by the landing.
