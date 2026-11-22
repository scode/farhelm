# Interrupted recovery guide still assumes no prior process remains

Reviewed commit: 44137d0659d28b9cadf90a8ffe96a65930035aad

## TLDR

The implementation guide justifies Restart without a confirmation by asserting the prior process is gone, although
legacy retirement may leave it running.

## Details

**other.** Source inspection. Main reference `SPEC_impl.md:3973`.

#4529 adds retirement as a second Interrupted route at SPEC_impl.md:3953–3956 and proto/src/lib.rs:865–869. The card
paragraph in that same Runtime state section still says no process remains at SPEC_impl.md:3973. SPEC.md:3405–3417 and
SPEC_impl.md:2589–2613 explicitly accept treating failed retirement as ended and the possibility of two agents. Reload
never looks up old panes (core.rs:6744–6748), converts eligible rows without a sentinel (6873–6887), and a later Restart
can open a host-kind generation for an old row (12728–12749). The GPUI card offers Restart from the drawn offer and
action at card.rs:455–472. No independent extinction premise appears there. Explain the lost terminal and use the
bounded legacy exception to qualify process state. Bucket: other for the documentation mismatch. Confidence:
source-supported, no prior-process survival or runtime effect observed. for the guide mismatch. The explicitly accepted
duplicate-run residual is set aside as a standalone runtime defect. An observed or source-established interference route
would be needed for a user-work-loss consequence. Possible coverage: l2 for editing the same card paragraph. Recovery
and stop interfaces.

Review evidence is retained under run `4d6e9e3b-26c2-4fd0-aeb6-997bfc824377` as `recovery_contract/l7`.
