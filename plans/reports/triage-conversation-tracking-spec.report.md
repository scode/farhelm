### What this was about

Restart uses the conversation Farhelm has recorded, which can lag behind the conversation the agent is actually in. Six
review findings described different ways that can happen. The maintainer chose a specification clarification, with one
draft PR per finding and no code changes.

### Things you should know

The specification now distinguishes three cases. Restart using the previous conversation briefly after a conversation
switch is acceptable. A new conversation Farhelm never learns of, leaving it undiscoverable long term, is a defect
mitigated by the existing session-tracking notifications. When tracking silently stops after Farhelm already recorded a
conversation, continuing to resume that old conversation is an accepted consequence of the tracking bug, rather than a
separate defect. The underlying tracking bugs are still to be avoided.

The six closures preserve the maintainer's recorded assignments: a report published during Restart, a transient report
recording failure and a reboot before a report is applied fall under the brief-window case; losing a new Codex selection
while withdrawing an unavailable old one falls under the never-learned case; an old report replayed across an upgrade
and a transcript read error fall under the accepted consequence of broken tracking. The Codex withdrawal case can leave
Restart unavailable rather than offering the previous conversation. None needed the plan's fallback for a case that did
not fit its recorded assignment.

The old promise that Restart always resumes the conversation the agent was actually in is reconciled with these limits.
The earlier crash/coincident-fault paragraph now points to the separate conversation-tracking rule, while its
process-cleanup and host-destination exceptions retain their original conditions. Database corruption, bricking an
installation and similar lasting damage remain unacceptable. The partial-integration allowance for other harnesses is
unchanged; it only gains a reference to the tracking limits.

SPEC_impl.md was checked for contradictions and needed no edit. Each finding is removed from the feedback queue in its
own commit, with its assessment and decision retained in the triage ledger and its execution linked to its own PR.

### Open questions and possible follow-ups

None blocks this scope. The reported runtime mechanisms are unchanged and were not reproduced. This work does not claim
to recover a discarded report, strengthen report admission, or make notifications detect every silent tracking failure.
A stronger guarantee would require a separate maintainer decision and implementation.

### The PRs

- [#2038: define conversation tracking limits](https://github.com/scode/farhelm/pull/2038/changes): the specification
  change and closure of the report-published-during-Restart finding.
- [#2044: delayed conversation report](https://github.com/scode/farhelm/pull/2044/changes): closes the transient
  recording-failure finding under #2038.
- [#2045: pending report at reboot](https://github.com/scode/farhelm/pull/2045/changes): closes the reboot-window
  finding.
- [#2046: lost Codex conversation](https://github.com/scode/farhelm/pull/2046/changes): closes the concurrent old-record
  withdrawal/new-selection finding.
- [#2047: conversation replay across an upgrade](https://github.com/scode/farhelm/pull/2047/changes): closes the
  mixed-version old-report finding.
- [#2048: transcript read error](https://github.com/scode/farhelm/pull/2048/changes): closes the stale-selection finding
  after an I/O refusal.

All six are open drafts in that linear order, the first based on main and each later PR on the preceding bookmark. The
executor marked none ready and merged none.

### Checks run, reused and skipped

- Ran changed-file `dprint check` and the diff whitespace check again after rebasing all six drafts onto main at
  `3c27aa5d`. Both passed. A separate documentation pass covered every touched Markdown file. Final inspection confirmed
  exactly one outcome per PR, correct full change IDs/bookmarks/PR links, and no `plans/` changes.
- Reused the previous executor's per-unit formatting, source reviews and message cold reads. Every incoming main diff
  was read: terminal-file opening, power-loss limits, trust for locally configured SSH values, hand-edited service
  guarantees, watcher policy and discarded-findings bookkeeping concern other rules. Shared feedback-index conflicts
  involved neighboring removals; both sides' removals were preserved. The conversation-tracking policy, decisions and
  messages did not change, so their review coverage still applies. A fresh native resume reader independently checked
  the six local and published review units and found no discrepancy.
- Skipped Rust, JavaScript, browser, desktop, installer, website, release and fault-injection tests. These are ordinary
  Markdown and feedback-file removals with no executable examples or generated artifacts; the documentation-only rule
  requires relevant formatting checks, not runtime execution. No earlier runtime evidence was reused and no runtime
  improvement is claimed. No validation failed.

### Review gate outcome

Two required fresh-context gpt-6.1-sol reviews at high effort passed with no findings: one for the specification PR, and
the plan-permitted shared review of the five bookkeeping-only PRs. They checked the complete diffs and all six original
outcomes, preservation of the maintainer's three cases and lasting-damage prohibition, absence of contradictory
implementation-spec text, and each queue/ledger change. The second independently verified the open draft chain and
confirmed the first PR's final amendment only filled its Execution URL. These were static reviews, with no runtime
verification.

Each final commit/PR message passed its own fresh native gpt-6.1-sol reader at medium effort after clarification where
needed. Implementation stayed in the main agent under the no-workhorse requirement. Private galaxy-brain records retain
decisions, attempts, outcomes and exposed external usage; native/main usage and exact reported model attribution remain
unavailable. A second executor resumed the published drafts after the first returned its claim with the report gate
incomplete. The resumed run completed a fresh native resume check and the separate native report cold read before
delivery; no new source-review or runtime claims are inferred from those process checks.

### Landing

Landed as #2038, #2044, #2045, #2046, #2047 and #2048, bottom-up, the round's only plan. Since the stack's base, main
had gained only plan bookkeeping; nothing touched the specification, the triage ledger or the review feedback index.

#### Review

An independent fresh-context review found nothing blocking. It confirmed that all three of the maintainer's cases are in
the new Durability and resume paragraph, that each of the six findings is closed under the case its ledger entry
records, that the prohibition on lasting damage and the process-cleanup and host-destination exceptions are intact, that
SPEC_impl.md contradicts nothing, and that each PR removes only its own feedback file and index entry and changes only
its own Execution field.

#### Fixes made while landing

- The specification change had deleted the 2026-10-10 clause accepting that, after a crash at one exact moment or two
  coinciding rare faults, a session's recorded conversation may be wrong, so Restart may resume a different conversation
  of that agent. None of the three new cases covers a conversation that is neither the previous one nor one Farhelm
  stopped updating, so deleting the clause would have quietly turned an accepted outcome back into a defect. The plan
  asked for a reconciliation, not a removal. The clause is restored, now as an exception to Restart resuming the
  conversation the agent last reported, with a pointer to the new limits.
- Two of the findings (a report discarded by Restart's own drain, and one lost to a reboot) are closed under the
  brief-window case, yet Farhelm never learns the conversation the agent had switched to, which literally reads as the
  "never learns of" defect. The brief-window sentence now also accepts Farhelm continuing with the conversation Restart
  resumed, so those findings cannot be filed again under the defect case.
- The sentence applying the limits to every harness now says a resume they accept is not the lost user work that the
  partial-integration allowance excludes.

#### Left as is

- Two docs pages promise more than the specification now does. The page on how Farhelm adds its hooks to agents says a
  restart resumes the conversation you were actually in after `/clear` or `/new`, and the Codex page says Restart
  resumes the new conversation once Codex has finished its next reply. Both are true in the usual case but ignore the
  accepted brief window and stopped-tracking cases. A wording follow-up for the maintainer to decide.
- Nits: "by clicking" (the maintainer's words) does not name what is clicked, and the opening sentence of Durability and
  resume reads awkwardly after the edit.

#### Checks

- Run now: `dprint check` on SPEC.md after the fix; no conflict markers on main after the stack landed.
- Not run: anything else; only Markdown changed.
