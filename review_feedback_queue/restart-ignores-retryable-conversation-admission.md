# Restart can proceed while a replacement report remains retryable

Independent drop decision: a fresh `gpt-6.1-sol` high reviewer rejected the proposed exclusion. A single ordinary
admission failure, followed by a successful durable snapshot read, can leave Restart using the previous conversation.
The crash/coincident-fault acceptance does not clearly cover this remainder. Preserve it as a pre-existing discovery,
distinct from permanently settled transcript errors and the initial-listing publication race. The interleaving was not
reproduced.

Reviewed commit: 379b215ea30df2ac0a3c29f34a71858aed131578

## TLDR

If the latest conversation report cannot be admitted, Restart can still resume the previously recorded conversation.

## Details

Suggested bucket: `highest`. Confidence: confirmed control-flow possibility; no runtime reproduction. Source: dedicated
`gpt-6.1-sol` high adversarial review. Proposed drop: Propose exclusion as pre-existing. TRIAGE_OUTCOMES.md heading
restart-resume-older-conversation-report-drain-fails.md additionally records acceptance for the specified crash or
coincident-fault cases. Retained for independent drop review; do not generalize that acceptance to ordinary failures..

service/report_files.rs:251–259,526–531 restores a transiently failed report and returns without exposing an
unresolved-report result. service/core.rs:10100–10135 waits for capture_pass(true), then reads the previous durable
identity. Thus waiting for the watcher or drain does not guarantee successful admission. The same behavior exists in the
first parent; this commit preserves it.

Possible cover: TRIAGE_OUTCOMES.md:8022–8038; current SPEC.md:1040–1051. The specification exception was added by
fa0051cd7419e1c481c78d587a045c48957da50e; it is an acceptance change, not a code fix.

Status at frozen endpoint: Underlying drain behavior persists at pinned main; report_files.rs is unchanged since the
reviewed commit. The recorded acceptance now appears in SPEC.md. No code fixing commit identified.

Review coverage: full commit diff read: true. No commit-diff hunks remain unread. Initial aggregated output truncated
part of report_watch.rs; reopened 303–591 in bounded chunks before relying on it. Unrelated portions of the 13,089-line
TRIAGE_OUTCOMES.md were searched rather than read in full. Unchanged surrounding modules, complete vendor adapters other
than Codex, macOS FSEvents internals and all existing tests were not exhaustively read. No live-vendor timing or
platform execution was performed. Highest proposed exclusions remain pending independent review.

Runtime evidence: None, as requested by default. Static review only; no checkout or scratch files created, modified or
deleted. No helper agents spawned.
