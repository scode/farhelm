## What this was about

Farhelm calls Send feedback private, but a proxy trusted by the operating system can inspect the message and contact
information. Separately, a user can launch a session through a symlink outside the managed checkout root, later point
that symlink at an archived checkout, and open a terminal tab there. Emptying the trash can then delete the tab's files.
The specification's retained-session protection did not state that limit.

The maintainer chose to document both as consequences of deliberate local setup. The specification now states the
general rule: protect against untrusted hosts and agent output, and against mistakes a reasonable user might make by
accident; take deliberate setup outside Farhelm as the user's intent when the consequence follows from that setup. It
preserves the existing file examples, adds the proxy and retargeted-symlink examples, and keeps the remote-host boundary
unchanged. The feedback and trash sections state their respective limits directly.

## Things you should know

This changes the specification and closes the two triaged findings; product behavior is unchanged. Feedback continues to
honor the operating system's certificate and proxy choices. Trash deletion still does not discover a session's working
folder through a symlink the user retargeted after launch.

The website's Send feedback page, `website/src/content/docs/docs/using/send-feedback.md`, still says feedback goes
privately to the maintainer and is filed where only the maintainer can read it, without the OS-trusted proxy caveat. The
first-session page also refers to private feedback. These broader privacy descriptions remain untouched, as the plan
requires; the Send feedback page is the place to explain the limit if the maintainer chooses to revise the docs. The
website's trash section makes no contradictory retained-session protection promise.

## Open questions and possible follow-ups

No blocking questions. A website wording follow-up can bring its feedback privacy description into line with the
clarified specification. No transport or trash implementation change is proposed by this plan.

## The PRs

- [#2075: define local security limits in the spec](https://github.com/scode/farhelm/pull/2075/changes) — the deliberate
  local setup principle and OS-trusted proxy privacy limit.
- [#2077: clarify trash protection after symlink changes](https://github.com/scode/farhelm/pull/2077/changes) — the
  retargeted-symlink example and trash-protection limit, stacked on #2075.

Both PRs remain drafts. Each removes its feedback file and index entry and records its change, bookmark and PR in the
triage ledger.

## Checks run, reused and skipped

- Ran `dprint check SPEC.md TRIAGE_OUTCOMES.md review_feedback_queue/INDEX.md` successfully for each change, after the
  rebase, and after the final PR-link update. Inspected the complete diffs and made a separate documentation pass.
- Reused the source-review evidence across the rebase from 424d3b39 to d96041d5: the entire incoming diff only records
  another plan's claim in the queue. The final PR-link updates only fill the reviewed ledger's pending PR references.
- Skipped runtime, browser, Rust, installer and website-build checks: only ordinary Markdown specification and triage
  bookkeeping changed, with no executable examples or generated artifacts affected. No earlier runtime evidence is
  claimed for this work.

## Review gate's outcome

Each PR passed its own fresh-context gpt-6.1-sol review at high effort, through the required external harness. Both
reported no findings after reading the full plan, relevant triage decisions, changes and surrounding specification.
Commit messages and PR wording passed fresh-context cold reads. No scope departure or unresolved review finding.
