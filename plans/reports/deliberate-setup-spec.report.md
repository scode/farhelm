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

### Landing

Landed as #2075 and #2077, bottom-up, the round's only plan. Since the stack's base, main had gained plan claims, a
triage record and the planning batch that queued it; none touches the specification sections this edits.

#### Review

An independent fresh-context review found nothing blocking. The principle matches the agreed decision, including the
"reasonable user would hit the trigger by accident" test, and keeps all four earlier file examples. Both limits match
the code: feedback goes through the operating system's certificate store and proxy settings with no pinning, and trash
deletion compares only the recorded path and the path resolved at launch, so a symlink retargeted later is not seen. No
other SPEC.md or SPEC_impl.md sentence contradicts the change, and each PR removes only its own finding and changes only
its own Execution field.

#### Fixes made while landing

- The principle said deliberately reshaped files are not Farhelm's job, which a later triage could read as overriding
  protections the specification keeps for exactly such cases, or the rule that lasting damage is never accepted. It now
  closes by saying it never relaxes the remote-host boundary, removes a protection stated for a specific case (the
  foreign-replacement guard, the installer keeping a user's file, uninstall refusing unknown app-bundle entries), or
  accepts database corruption, a bricked installation or similar lasting damage.
- "A proxy the operating system trusts" was imprecise: an ordinary forwarding proxy cannot read HTTPS. Both places now
  say an intercepting proxy whose certificate the OS trusts, and the feedback limit follows the HTTPS sentence it
  qualifies.
- The retargeted-symlink example could be read as retargeting the symlink to a place outside the checkout root; it now
  describes a session launched through a symlink outside the root that the user later points at an archived checkout.
  The trash section's clause now speaks of a working folder reached through such a symlink.

#### Left as is

The docs' Send feedback page calls feedback private ("goes privately to the maintainer", filed "where only the
maintainer can read it"), in the same sense as SPEC.md's own unchanged wording. If the docs should mention the
intercepting-proxy limit, its HTTPS paragraph is the place; the plan left the docs untouched.

#### Checks

- Run now: `dprint check` on SPEC.md after the fixes and the conflict resolution between the two PRs; the combined diff
  was read in full.
- Not run: anything else; only Markdown changed.
