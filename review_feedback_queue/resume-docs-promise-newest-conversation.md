# Resume documentation promises more than the accepted tracking behavior

Reviewed commit: 75af240c09c7ecd5b81a0fcba9e03f204b0be0a7

## TLDR

Readers are told Restart cannot return to the conversation they cleared, although Farhelm's accepted behavior permits
exactly that.

## Details

Suggested bucket: `other`. Confidence: confirmed documentation mismatch; harmful reliance was not reproduced or
independently established. Source: separate adversarial trailing-review agent, documentation and bookkeeping scope.
Proposed drop: none.

`84520906bf06048485a45116103c8c11434b2182` changes the tracking contract, but leaves the user-facing explanation
unchanged. At the reviewed tip, `website/src/content/docs/docs/agents/codex.md:22–23` says Restart no longer resumes the
old conversation after `/clear` and resumes the new one after its next reply.
`website/src/content/docs/docs/how-it-works/agent-hook-injection.md:15–16` similarly promises the actual post-clear
conversation instead of the discarded one.

`SPEC.md:1761–1768` explicitly accepts a brief-window restart of the earlier conversation, continuing with that resumed
conversation, and continued use of an older recorded conversation when tracking silently stops updating.
`plans/reports/triage-conversation-tracking-spec.report.md:117–120` acknowledges the unresolved documentation mismatch.
Explain Restart in terms of the last recorded conversation and qualify the tracking limits. The website's editorial
rules at `website/EDITORIAL_RULES.md:12–16` require agreement with the specifications.

Possible cover: `introduction-promises-universal-reboot-resume.md` concerns unsupported harnesses after reboot, not
stale captures for supported conversation switches. Accepting the product limitation does not make the stronger docs
promise accurate. No matching Planned item, BUGS entry, ledger decision, or filter was found.

All 91 assigned documentation and bookkeeping commits were read completely, including deleted findings and transient
plans. Surrounding code and ledger sections were inspected selectively. External prior-review transcripts and private
runtime evidence were unavailable; report claims were checked for internal integrity, not independently authenticated.
No runtime tests were run.
