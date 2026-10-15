### What this was about

Setup and uninstall read Farhelm's service definitions to decide which installation owns a service. Two review findings
showed that manually edited definitions, using dollar signs or continued lines, could make that decision wrong and lead
to acting on another installation's service. The maintainer decided that hand editing voids those guarantees. This plan
records that decision in the specification and closes the two findings in separate PRs. No product code changed.

### Things you should know

The policy explicitly covers command-line `farhelm helm setup`, including its uninstall mode, `farhelm uninstall`, and
the hosts panel's setup, Update and uninstall. A hand-edited Farhelm unit can be attributed to the wrong installation,
so an operation may overwrite, stop or remove another installation's service. This is an accepted risk after manual
edits; the parser has not been fixed to handle those cases.

The specification's general refusal rule now points to this exception, and its existing service-type refusal is
explicitly outside the guarantees for hand-edited units. Guarantees for unedited units and the refusal to install at a
program path containing a dollar sign remain unchanged. Existing best-effort setup and drop-in text is retained. Each
triage entry keeps its assessment, decision and completion criteria; only Execution records the work and PR.

### Open questions and possible follow-ups

None identified within the accepted documentation-only scope.

### PRs

- [#2041](https://github.com/scode/farhelm/pull/2041/changes): document the hand-edit exception and close the
  dollar-decoding finding; draft against main.
- [#2049](https://github.com/scode/farhelm/pull/2049/changes): close the continuation-line finding under that policy;
  draft stacked on #2041.

### Checks run, reused and skipped

- Ran `dprint check SPEC.md TRIAGE_OUTCOMES.md review_feedback_queue/INDEX.md` for the policy PR, and changed-file
  dprint for the bookkeeping PR and their mechanical PR-link updates. A separate documentation pass covered every
  touched file.
- Reused no runtime evidence. Read the complete incoming diff since the claimed base `617ee2e0`: two other plans' report
  deliveries and queue states only, with no change to the service policy or its source references. Rebased both commits
  onto `e79840c6`; formatting evidence still applies to the unchanged Markdown content.
- Skipped Rust, JavaScript, browser, desktop, installer, provisioning and website tests: ordinary Markdown edits and
  feedback-file removals cannot change executable behavior or generated artifacts. This work does not reproduce or claim
  to fix the parser behavior reported by either finding.

### Review gate outcome

Each PR received its required fresh-context gpt-6.1-sol review at high effort through codex exec, with the full plan and
triage outcomes. Both reported no findings; their scope and coverage were checked against the diffs and relevant unit
readers. Commit and PR wording received separate fresh-context cold reads; the second message was rewritten to explain
the accepted risk. The report receives its required native cold read before delivery.

Implementation stayed in the main session under the no-workhorse requirement. Wording readers used gpt-6.1-sol at medium
effort. Private galaxy-brain records retain source-review decisions and external per-turn usage; exact reported model
attribution and native/main usage are unavailable. The two PRs touch no files under `plans/`, remain drafts and are not
merged by the executor.

### Landing

Landed as #2041 and #2049, last of the round's three spec-only plans. The rebase of #2049 met a conflict in the review
feedback index, where triage-power-loss-spec (#2043) had removed the neighbouring entry; both removals were kept.

#### Review

An independent fresh-context review found nothing blocking. It traced every place Farhelm reads a service unit:
`farhelm helm setup` and its uninstall mode, `farhelm uninstall`, the hosts panel on the helm's own machine (which reads
the unit only to word its refusal), and the hosts panel's remote setup, Update and uninstall. The covered operations
match the code, none is missing, and the refusal to install at a program path containing `$` is decided from the install
path, not a unit, so the exception does not reach it.

#### Fixes made while landing

Two SPEC.md passages still read as absolute rules that the new exception voids, which is how the continuation-line
finding arose:

- The 2026-10-10 service-type paragraph now says a type change is a hand edit, so its refusal is best effort: an edit
  that hides the type or start command from Farhelm's reading may not be refused.
- The uninstall section's rule for recognising services now says a hand-edited unit is outside that guarantee and may be
  attributed to another installation, with a link to the exception.

#### Left as is

Nits: the refusal rule's parenthetical says "where this section says otherwise" though the hand-edit exception sits in
another section (the link covers it), and the new paragraph sits in a section whose opening is about remote hosts, as
the triage outcome asked.

#### Checks

- Run now: `dprint check` on SPEC.md after the fixes and on the index after the conflict resolution; no conflict markers
  on main after all three plans landed.
- Not run: anything else; only Markdown changed.
