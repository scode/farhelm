### What this was about

Clone and Replace with open the new-session dialog pre-filled from the original session, its name included. If you then
picked a fresh GitHub checkout (`gh:owner/repo`) as the destination, Farhelm treated that copied name as one you had
typed. It named the checkout directory after it, found the directory taken (usually by the original session's own
checkout), and refused the launch: "a directory with that checkout name already exists; choose another title". For an
original session that was itself a checkout of the same repository, this happened every time. Three TODO entries
reported it; the PR removes all three (two described this bug, the third is covered by your first decision below).

During planning you decided:

- Clone keeps sharing the original session's working copy, as the spec requires; a separate checkout only happens when
  you pick one. The TODO entry asking for clones of checkout sessions to get their own working copy is removed as
  working as intended.
- When the destination is a fresh checkout and the copied name is unedited, the new session counts as unnamed. It gets
  the next free `repo-N`, as a new session with no name would.
- The name field never shows information that will be ignored. It goes empty and shows the `repo-N` as placeholder text.
- Clone and Replace with are fixed the same way. `farhelm agent clone` is untouched.

What the plan did: the dialog now ignores a copied, unedited name whenever a fresh checkout is the destination. It
decides this each time it reads the name, rather than clearing the field when a repository is picked. As a result,
switching back to an ordinary folder brings the copied name back. A name you type is used as typed, and a taken one is
still refused. No helm or supervisor change was needed: an empty name is already how an unnamed checkout is requested.

### Things you should know

- **Two cases change that used to launch.** A session renamed after it was created no longer matches its checkout's
  directory, so cloning it into a fresh checkout used to succeed with a checkout named after the new name. Cloning an
  ordinary (non-checkout) session into a fresh checkout likewise used its name. Both now get `repo-N` unless you type a
  name. The PR adds this to SPEC.md explicitly, so it should not read as a regression.
- **The placeholder also shows on a plain New dialog.** Any checkout launch with an empty name field shows the `repo-N`
  it will get as placeholder text. The PR's SPEC.md text states that general rule.
- **SPEC_impl.md gained one sentence.** It notes that the title is the exception to the rule that an untouched cloned
  field submits the original text. It also says why the composer decides at read time.
- **A pre-existing warning showed up during testing.** While a fresh checkout replaced another session, the helm's log
  once showed its periodic refresh of the session list failing, with the supervisor reporting "the session's
  fresh-checkout provenance does not match its registry evidence". Nothing user-visible followed; the next refresh
  succeeded. The same warning appears in an earlier full browser run from 2026-09-30, in the existing replacement test,
  before this change existed. It looks like a brief window during a checkout replace, not something this change
  introduced. It was not investigated further.
- **No docs page needed changing.** The docs website has no page describing Clone or Replace with into a `gh:` checkout.

### Open questions and possible follow-ups

- The provenance warning above may deserve its own look. It had no visible effect in these runs, but the supervisor
  reports it as an internal error, which suggests a brief inconsistency while a replace is in progress.

### The PRs

- #1482 (draft): fix: name sessions cloned into a fresh checkout repo-N.

### Checks

Run now:

- `cargo fmt --all -- --check`, `cargo clippy -p farhelm-ui --all-targets -- -D warnings`, and
  `cargo check -p farhelm-ui --features desktop`: clean.
- Recorded nextest run 5ad5157b: the composer's unit tests (30), including the new one for the title rule. All passed.
- Recorded browser runs on Chromium and WebKit, covering: a new test driving Clone and Replace with in the dialog
  against a stubbed checkout preview; an existing Replace with test whose expected directory changed with this fix; and
  a new test doing a real Replace with into a real local checkout.
  - Run 5eb74878 failed only on a wrong expectation in the new tests (how an empty name is encoded in the request). The
    tests were corrected.
  - Run 48d419e8 passed 7 of 8. The WebKit real-clone test passed every assertion but hit the 60-second test limit on a
    heavily loaded machine. The test was slimmed by setting up its source session through the API.
  - Runs 764482c9 (2/2) and 82192324 (6/6, after the review fixes) passed.
- `dprint check` on the changed files, `python3 releasing/check-changelog.py format`, and the test-sleep check (zero
  unannotated delays).

Skipped:

- The workspace Rust battery, doctests, desktop runtime, and installer checks. The change is confined to the web UI's
  new-session dialog, its specs, and two browser spec files.
- The rest of the browser suite. The change touches only how the dialog reads its name field, and the affected specs
  were run.

### Review gate

A fresh-context Opus 5.5 reviewer at high effort reviewed adversarially. It found no correctness defect. It confirmed
that every place the dialog reads the name for a launch goes through the new rule. That matters because the dialog shows
a preview of the checkout path before Launch, and the helm refuses a launch whose name differs from the one the preview
was computed for.

It raised two documentation gaps, both fixed: the SPEC_impl.md drift, and the New-dialog placeholder missing from
SPEC.md. It made three test suggestions:

- A clear failure when the real-clone test's host is missing, and optionally building that test's source session through
  the dialog instead of the API. The first part was fixed. The second was declined: that version exceeded the time limit
  under load, and the test is about Replace with.
- Re-checking the preview's empty name after switching away from a checkout and back. Fixed.
- An optional extra test for entering a checkout through a saved repository setup. Declined: the reviewer's own reading
  found that path never touches the name.
