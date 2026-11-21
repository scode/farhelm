# An interrupted upload can stay unreported while a reopened session has no row

Reviewed commit: 9864b874b3ac36670cb52a566ecbc08610a71430

## TLDR

An interrupted upload can go unreported while the reopened session is usable but still missing from the list.

## Details

**other**. Confirmed by source and contract inspection.

The old terminal carries interrupted-upload results (attachments.rs:665–690). TerminalSet suppresses delivery while
`restore.is_some()` even after input makes late restoration impossible (set.rs:527–545,590–604,618–628). The first
refusal retries the selection with the old filters. A refused naming-only query retains the old accepted helm query, so
it may keep excluding the opened session even when later data pushes succeed
(filters.rs:215–221,264–274,312–334,390–453. Helm ui_channel.rs:887–906. Sessions.rs:891–914). The attachment stream is
independent. The parent attachments.rs:464–498 delivered immediately to the primary terminal on this route. Confirmed
source behavior conditional on list-query refusal, estimated other since the notice remains available for later delivery
and no source-supported loss of the original file is established. Cross-group: shell-presentation/f2 correctly observes
that a failed refresh alone preserves rows the helm has already sent (helm ui_channel.rs:468–484). That does not create
the never-delivered row in this finding, and the neighbor's cap-only label issue concerns a different visible effect. No
cap omission is needed or relied on here. SPEC.md:3461–3463 requires visible upload failures. Possible_cover:
SPEC_impl.md:486–493 permits waiting to settle a potential restore, but after the user moves on a late restore cannot
happen. SPEC.md:5351 covers in-flight uploads at UI reset with a generic report. The session-switch case here is
independent of that reset. The docs-only follow-up a315f83db44cf741961ff7ed79712b92755fd0f1 and the later primary Shell
label preserve the guard at pinned tip 8bcca67df1afe6eb0f184ef73b9952ed7ebb329f set.rs:618–628. Source only.

Review evidence is recorded under run `328737fe-cec4-4461-9ba9-d042969d54c1` as `tabs/f2`.
