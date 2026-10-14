# Covered candidate: failed confirmation restoration is lost after reload

Independent drop decision: a fresh `gpt-6-astra` high reviewer rejected the exclusion as ambiguous. The explicit
silent-persistence exception names this preference, but describes losing convenience and falling back to defaults. Here
an older persisted opt-out survives a failed attempt to turn safeguards back on and returns after reload. Retain that
acceptance question; subsequent accidental destructive activation was not reproduced.

Reviewed commit: e440591fd6e95188fe171d0e46d4c27703477386

## TLDR

If turning confirmations back on fails to save, reloading can silently restore first-click destructive actions.

## Details

Suggested bucket: `highest`. Confidence: confirmed. Source: dedicated `gpt-6-astra` high adversarial review. Proposed
drop: Proposed covered by the explicit best-effort preference persistence contract. Independent drop review remains
required; no independent reviewer was spawned under this task's prohibition..

crates/farhelm-ui/src/settings.rs:120–125 immediately changes the local choice and queues persistence. api.rs:753–764
and 919–933 neither automatically retry nor visibly report a failed write. Suppose true was already persisted, the user
unchecks the switch, and that PUT fails before committing. The current client asks again, but a fresh browser load or
app restart loses the process-local dirty queue (api.rs:809–814) and reads persisted true. A subsequent Delete, Replace
or tab Close can therefore destroy session state/processes on a click the user expected to open a question.
Reauthentication within the existing process is protected by api.rs:869–895. The failure/reload behavior is statically
confirmed; accidental destructive activation was not reproduced. Retained as potentially highest for independent
coverage review, not asserted as an uncovered specification violation.

Possible cover: Historical SPEC.md, Errors and diagnostics, 2092–2097 explicitly includes the session-confirmation
opt-out among silent persistence failures. SPEC_impl.md:151–159 specifies process-local replay and next-launch failure
behavior. Caveat for the independent reviewer: the explanation mentions falling back to defaults, whereas this trigger
retains an older persisted true value.

Status at frozen endpoint: Behavior and acceptance persist at pinned main. Settings is unchanged; api.rs retains
SessionConfirmations in the same queue and failed-write policy. Current SPEC.md:2112–2117 retains the explicit
exception. No fixing commit.

Review coverage: full commit diff read: true. No commit diff hunks remain unread. Initial aggregate output truncation
was resolved by smaller rereads. Unchanged files were inspected selectively, not exhaustively. Most TRIAGE_OUTCOMES.md
entry bodies and queue files unrelated by the pinned index were not read. Backend lifecycle implementations beyond the
cited guard paths were not exhaustively re-reviewed. No Sol reports or working-copy queue additions were read.
Independent review of the proposed highest-candidate drop remains pending.

Runtime evidence: None, as requested. Conclusions are static traces. Applied the supplied test-authoring checklist to
changed tests and fixtures. No checkout files or scratch artifacts were created, modified or deleted; no fetch, rebase
or helper agents.
